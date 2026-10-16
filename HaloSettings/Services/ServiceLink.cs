using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace HaloSettings.Services;

/// <summary>
/// Loopback link to the Halo tray service: newline-delimited JSON over TCP.
/// The service writes its port and a per-run token to %APPDATA%\Halo\ipc.json.
/// Reconnects on its own when the service restarts. Events are raised on a background thread.
/// </summary>
public sealed class ServiceLink : IDisposable
{
    private static readonly string EndpointPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Halo", "ipc.json");

    private static readonly TimeSpan RetryDelay = TimeSpan.FromMilliseconds(800);
    private static readonly TimeSpan ConnectTimeout = TimeSpan.FromMilliseconds(500);

    private readonly CancellationTokenSource _cts = new();
    private readonly object _gate = new();
    private StreamWriter? _writer;

    public event Action<JsonObject, JsonObject>? StateReceived;
    public event Action<byte[]>? LedsReceived;
    public event Action<bool>? OnlineChanged;

    public void Start() => _ = Task.Run(() => RunAsync(_cts.Token));

    public void Send(JsonObject message)
    {
        lock (_gate)
        {
            if (_writer is null)
            {
                return;
            }
            try
            {
                _writer.WriteLine(message.ToJsonString());
            }
            catch (Exception e) when (e is IOException or ObjectDisposedException)
            {
                _writer = null;
            }
        }
    }

    public void Dispose()
    {
        _cts.Cancel();
        _cts.Dispose();
    }

    private async Task RunAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                await SessionAsync(ct);
            }
            catch (Exception e) when (!ct.IsCancellationRequested)
            {
                // A missing endpoint file just means the service is not running yet; anything else is worth a log line.
                if (e is not FileNotFoundException and not SocketException)
                {
                    App.Log(e);
                }
                Debug.WriteLine($"Halo service link: {e.Message}");
            }
            SetWriter(null);
            OnlineChanged?.Invoke(false);
            try
            {
                await Task.Delay(RetryDelay, ct);
            }
            catch (OperationCanceledException)
            {
                return;
            }
        }
    }

    private async Task SessionAsync(CancellationToken ct)
    {
        var endpoint = JsonNode.Parse(await File.ReadAllTextAsync(EndpointPath, ct))
                       ?? throw new JsonException("empty endpoint file");
        var port = endpoint["port"]?.GetValue<int>() ?? throw new JsonException("no port");
        var token = endpoint["token"]?.GetValue<string>() ?? throw new JsonException("no token");

        using var client = new TcpClient { NoDelay = true };
        using (var connect = CancellationTokenSource.CreateLinkedTokenSource(ct))
        {
            connect.CancelAfter(ConnectTimeout);
            await client.ConnectAsync(IPAddress.Loopback, port, connect.Token);
        }

        var stream = client.GetStream();
        var writer = new StreamWriter(stream, new UTF8Encoding(false)) { AutoFlush = true, NewLine = "\n" };
        await writer.WriteLineAsync(new JsonObject { ["t"] = "hello", ["token"] = token }.ToJsonString());
        SetWriter(writer);
        OnlineChanged?.Invoke(true);

        using var reader = new StreamReader(stream, Encoding.UTF8);
        while (!ct.IsCancellationRequested)
        {
            var line = await reader.ReadLineAsync(ct);
            if (line is null)
            {
                return;
            }
            Dispatch(line);
        }
    }

    private void SetWriter(StreamWriter? writer)
    {
        lock (_gate)
        {
            _writer = writer;
        }
    }

    private void Dispatch(string line)
    {
        JsonNode? node;
        try
        {
            node = JsonNode.Parse(line);
        }
        catch (JsonException)
        {
            return;
        }

        switch (node?["t"]?.GetValue<string>())
        {
            case "state" when node["config"] is JsonObject config && node["status"] is JsonObject status:
                StateReceived?.Invoke(config, status);
                break;
            case "leds" when node["c"] is JsonArray leds:
                LedsReceived?.Invoke(FlattenLeds(leds));
                break;
        }
    }

    private static byte[] FlattenLeds(JsonArray leds)
    {
        var rgb = new byte[leds.Count * 3];
        for (var i = 0; i < leds.Count; i++)
        {
            if (leds[i] is not JsonArray c || c.Count < 3)
            {
                continue;
            }
            for (var k = 0; k < 3; k++)
            {
                rgb[i * 3 + k] = (byte)Math.Clamp(c[k]?.GetValue<int>() ?? 0, 0, 255);
            }
        }
        return rgb;
    }
}
