using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.Text.Json.Nodes;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using HaloSettings.Services;
using Microsoft.UI.Dispatching;
using Windows.UI;

namespace HaloSettings.ViewModels;

public sealed partial class OpenRgbDevice : ObservableObject
{
    public OpenRgbDevice(string name, bool isOn)
    {
        Name = name;
        _isOn = isOn;
    }

    public string Name { get; }

    [ObservableProperty]
    private bool _isOn;
}

/// <summary>
/// Mirrors the service config. Edits are pushed back as a whole config; the last config received
/// is kept as JSON so fields this window does not know about survive the round trip.
/// </summary>
public sealed partial class MainViewModel : ObservableObject
{
    private static readonly string[] Modes = ["screen", "solid", "breathe", "rainbow", "aurora"];

    /// <summary>Echoes from the service are ignored this long after a local edit, so sliders never jump back.</summary>
    private static readonly TimeSpan EchoGuard = TimeSpan.FromSeconds(1);

    private static readonly HashSet<string> Persisted =
    [
        nameof(Enabled), nameof(Brightness), nameof(ModeIndex), nameof(MonitorIndex), nameof(Smoothing),
        nameof(Saturation), nameof(Depth), nameof(FpsIndex), nameof(Letterbox), nameof(Speed), nameof(Color),
        nameof(Left), nameof(Top), nameof(Right), nameof(Bottom), nameof(Reverse), nameof(Autostart),
        nameof(OpenRgbEnabled),
    ];

    private readonly ServiceLink _link = new();
    private readonly DispatcherQueue _ui;
    private JsonObject _config = new();
    private List<int> _monitorIds = [];
    private bool _applying;
    private DateTime _lastEdit = DateTime.MinValue;

    public MainViewModel(DispatcherQueue ui)
    {
        _ui = ui;
        _link.StateReceived += (config, status) => _ui.TryEnqueue(() => Apply(config, status));
        _link.LedsReceived += leds => _ui.TryEnqueue(() => LedsUpdated?.Invoke(leds));
        _link.OnlineChanged += online => _ui.TryEnqueue(() => IsOnline = online);
        _link.Start();
    }

    /// <summary>Live strip colours, flat RGB, raised on the UI thread.</summary>
    public event Action<byte[]>? LedsUpdated;

    public ObservableCollection<string> Monitors { get; } = [];
    public ObservableCollection<OpenRgbDevice> OpenRgbDevices { get; } = [];

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ProblemOpen), nameof(OfflineOpen))]
    private bool _isOnline;

    [ObservableProperty]
    private bool _enabled;

    [ObservableProperty]
    private double _brightness;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsScreen), nameof(IsColor), nameof(HasSpeed), nameof(ModeHint))]
    private int _modeIndex;

    [ObservableProperty]
    private int _monitorIndex;

    [ObservableProperty]
    private double _smoothing;

    [ObservableProperty]
    private double _saturation;

    [ObservableProperty]
    private double _depth;

    [ObservableProperty]
    private int _fpsIndex;

    [ObservableProperty]
    private bool _letterbox;

    [ObservableProperty]
    private double _speed;

    [ObservableProperty]
    private Color _color;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(LedCount))]
    private double _left;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(LedCount))]
    private double _top;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(LedCount))]
    private double _right;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(LedCount))]
    private double _bottom;

    [ObservableProperty]
    private bool _reverse;

    [ObservableProperty]
    private bool _autostart;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(OpenRgbNoteOpen))]
    private bool _openRgbEnabled;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(OpenRgbNoteOpen))]
    private string _openRgbNote = "";

    [ObservableProperty]
    private string _deviceTitle = "Подключение…";

    [ObservableProperty]
    private string _deviceDetail = "";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ProblemOpen))]
    private string _problem = "";

    public bool IsScreen => ModeIndex == 0;
    public bool IsColor => ModeIndex is 1 or 2;
    public bool HasSpeed => ModeIndex is 2 or 3 or 4;
    public bool ProblemOpen => IsOnline && Problem.Length > 0;
    public bool OfflineOpen => !IsOnline;
    public bool OpenRgbNoteOpen => OpenRgbEnabled && OpenRgbNote.Length > 0;

    public string ModeHint => ModeIndex switch
    {
        0 => "Лента повторяет края экрана",
        1 => "Один ровный цвет",
        2 => "Цвет плавно дышит",
        3 => "Радуга бежит по кругу",
        _ => "Медленные переливы бирюзы и фиолета",
    };

    public StripLayout Layout => new((int)Left, (int)Top, (int)Right, (int)Bottom, Reverse);

    public string LedCount => Plural(Layout.Total);

    [RelayCommand]
    private void Rescan() => _link.Send(new JsonObject { ["t"] = "rescan" });

    [RelayCommand]
    private void QuitService() => _link.Send(new JsonObject { ["t"] = "quit" });

    [RelayCommand]
    private void StartService()
    {
        var exe = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "Halo.exe"));
        if (!File.Exists(exe))
        {
            Problem = $"Не найден {exe}";
            return;
        }
        Process.Start(new ProcessStartInfo(exe, "--background") { UseShellExecute = false });
    }

    protected override void OnPropertyChanged(PropertyChangedEventArgs e)
    {
        base.OnPropertyChanged(e);
        if (!_applying && e.PropertyName is { } name && Persisted.Contains(name))
        {
            Push();
        }
    }

    private static string Plural(int n)
    {
        var word = (n % 10, n % 100) switch
        {
            (1, not 11) => "светодиод",
            (>= 2 and <= 4, < 12 or > 14) => "светодиода",
            _ => "светодиодов",
        };
        return $"{n} {word}";
    }

    private static double Num(JsonNode? node, double fallback) =>
        node is JsonValue v && v.TryGetValue(out double d) ? d : fallback;

    private static bool Flag(JsonNode? node, bool fallback) =>
        node is JsonValue v && v.TryGetValue(out bool b) ? b : fallback;

    private static string Text(JsonNode? node) =>
        node is JsonValue v && v.TryGetValue(out string? s) ? s ?? "" : "";

    private void Apply(JsonObject config, JsonObject status)
    {
        _applying = true;
        try
        {
            ApplyStatus(status);
            if (DateTime.UtcNow - _lastEdit > EchoGuard)
            {
                ApplyConfig(config);
            }
        }
        finally
        {
            _applying = false;
        }
    }

    private void ApplyConfig(JsonObject c)
    {
        _config = c;
        Enabled = Flag(c["enabled"], true);
        Brightness = Math.Round(Num(c["brightness"], 0.6) * 100);
        ModeIndex = Math.Max(0, Array.IndexOf(Modes, Text(c["mode"])));
        Speed = Num(c["speed"], 1);
        if (c["color"] is JsonArray rgb && rgb.Count == 3)
        {
            Color = Color.FromArgb(255, (byte)Num(rgb[0], 255), (byte)Num(rgb[1], 170), (byte)Num(rgb[2], 90));
        }

        var s = c["screen"] as JsonObject ?? [];
        var monitor = (int)Num(s["monitor"], 0);
        MonitorIndex = Math.Max(0, _monitorIds.IndexOf(monitor));
        Smoothing = Math.Round(Num(s["smoothing"], 0.45) * 100);
        Saturation = Math.Round(Num(s["saturation"], 1.15) * 100);
        Depth = Math.Round(Num(s["depth"], 0.12) * 100);
        FpsIndex = Num(s["fps"], 30) >= 45 ? 1 : 0;
        Letterbox = Flag(s["letterbox"], true);

        var l = c["layout"] as JsonObject ?? [];
        Left = Num(l["left"], 14);
        Top = Num(l["top"], 26);
        Right = Num(l["right"], 14);
        Bottom = Num(l["bottom"], 0);
        Reverse = Flag(l["reverse"], false);

        Autostart = Flag(c["autostart"], false);
        var o = c["openrgb"] as JsonObject ?? [];
        OpenRgbEnabled = Flag(o["enabled"], false);
        var selected = (o["devices"] as JsonArray ?? []).Select(Text).ToHashSet();
        foreach (var d in OpenRgbDevices)
        {
            d.IsOn = selected.Contains(d.Name);
        }
        OnPropertyChanged(nameof(Layout));
    }

    private void ApplyStatus(JsonObject s)
    {
        if (s["device"] is JsonObject d)
        {
            DeviceTitle = $"Skydimo {Text(d["model"])}";
            DeviceDetail = $"{Text(d["port"])}, {LedCount}";
        }
        else
        {
            DeviceTitle = "Лента не подключена";
            DeviceDetail = Text(s["note"]) is { Length: > 0 } note ? note : "Ищу ленту…";
        }

        var problems = new[] { Text(s["capture_error"]) }.Where(p => p.Length > 0);
        Problem = string.Join("\n", problems);
        OpenRgbNote = Text(s["openrgb_note"]);

        var monitors = (s["monitors"] as JsonArray ?? []).OfType<JsonObject>().ToList();
        var ids = monitors.Select(m => (int)Num(m["index"], 0)).ToList();
        if (!ids.SequenceEqual(_monitorIds))
        {
            _monitorIds = ids;
            Monitors.Clear();
            foreach (var m in monitors)
            {
                var primary = Flag(m["primary"], false) ? ", основной" : "";
                Monitors.Add($"Монитор {Num(m["index"], 0) + 1}, {Num(m["width"], 0)}×{Num(m["height"], 0)}{primary}");
            }
            var current = (int)Num((_config["screen"] as JsonObject)?["monitor"], 0);
            MonitorIndex = Math.Max(0, _monitorIds.IndexOf(current));
        }

        var names = (s["openrgb"] as JsonArray ?? []).Select(Text)
            .Where(n => !n.Contains("skydimo", StringComparison.OrdinalIgnoreCase)).ToList();
        if (!names.SequenceEqual(OpenRgbDevices.Select(x => x.Name)))
        {
            var selected = ((_config["openrgb"] as JsonObject)?["devices"] as JsonArray ?? []).Select(Text).ToHashSet();
            foreach (var old in OpenRgbDevices)
            {
                old.PropertyChanged -= OnDeviceToggled;
            }
            OpenRgbDevices.Clear();
            foreach (var n in names)
            {
                var device = new OpenRgbDevice(n, selected.Contains(n));
                device.PropertyChanged += OnDeviceToggled;
                OpenRgbDevices.Add(device);
            }
        }
    }

    private void OnDeviceToggled(object? sender, PropertyChangedEventArgs e)
    {
        if (!_applying)
        {
            Push();
        }
    }

    private void Push()
    {
        if (!IsOnline)
        {
            return;
        }
        var c = (JsonObject)_config.DeepClone();
        c["enabled"] = Enabled;
        c["brightness"] = Brightness / 100.0;
        c["mode"] = Modes[Math.Clamp(ModeIndex, 0, Modes.Length - 1)];
        c["speed"] = Speed;
        c["color"] = new JsonArray(Color.R, Color.G, Color.B);

        var s = c["screen"] as JsonObject ?? [];
        if (MonitorIndex >= 0 && MonitorIndex < _monitorIds.Count)
        {
            s["monitor"] = _monitorIds[MonitorIndex];
        }
        s["smoothing"] = Smoothing / 100.0;
        s["saturation"] = Saturation / 100.0;
        s["depth"] = Depth / 100.0;
        s["fps"] = FpsIndex == 1 ? 60 : 30;
        s["letterbox"] = Letterbox;
        c["screen"] = s;

        c["layout"] = new JsonObject
        {
            ["left"] = (int)Left,
            ["top"] = (int)Top,
            ["right"] = (int)Right,
            ["bottom"] = (int)Bottom,
            ["reverse"] = Reverse,
        };
        c["autostart"] = Autostart;

        var o = c["openrgb"] as JsonObject ?? [];
        o["enabled"] = OpenRgbEnabled;
        o["devices"] = new JsonArray(OpenRgbDevices.Where(d => d.IsOn).Select(d => (JsonNode?)d.Name).ToArray());
        c["openrgb"] = o;

        _config = c;
        _lastEdit = DateTime.UtcNow;
        _link.Send(new JsonObject { ["t"] = "set", ["config"] = c.DeepClone() });
        OnPropertyChanged(nameof(Layout));
    }

    public void Shutdown() => _link.Dispose();
}
