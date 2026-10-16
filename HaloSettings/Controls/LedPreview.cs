using HaloSettings.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Windows.UI;

namespace HaloSettings.Controls;

/// <summary>
/// A monitor drawn at the centre with the strip's live colours glowing around its edges.
/// The screen itself is always dark — it is a picture of a monitor, not app chrome.
/// </summary>
public sealed partial class LedPreview : Grid
{
    private const double ScreenWidth = 236;
    private const double ScreenHeight = 134;
    private const double GlowSize = 70;
    private const double GlowOffset = 12;
    private const byte GlowAlpha = 150;

    private readonly Canvas _glow = new() { IsHitTestVisible = false };
    private readonly Canvas _chips = new() { IsHitTestVisible = false };
    private readonly Border _screen;
    private readonly TextBlock _caption;
    private readonly List<(Ellipse Glow, GradientStop Inner, GradientStop Outer, Ellipse Chip, SolidColorBrush ChipBrush)> _leds = [];
    private IReadOnlyList<Slot> _slots = [];

    public LedPreview()
    {
        Height = 210;
        _caption = new TextBlock
        {
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = new SolidColorBrush(Color.FromArgb(150, 255, 255, 255)),
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        };
        _screen = new Border
        {
            Width = ScreenWidth,
            Height = ScreenHeight,
            CornerRadius = new CornerRadius(10),
            BorderThickness = new Thickness(1),
            BorderBrush = new SolidColorBrush(Color.FromArgb(40, 255, 255, 255)),
            Background = new LinearGradientBrush
            {
                StartPoint = new Point(0, 0),
                EndPoint = new Point(0, 1),
                GradientStops =
                {
                    new GradientStop { Color = Color.FromArgb(255, 30, 30, 38), Offset = 0 },
                    new GradientStop { Color = Color.FromArgb(255, 10, 10, 13), Offset = 1 },
                },
            },
            Child = _caption,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 0, 22),
        };
        var stand = new StackPanel
        {
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, ScreenHeight + 4, 0, 0),
            IsHitTestVisible = false,
            Children =
            {
                new Rectangle { Width = 26, Height = 14, Fill = StandBrush(70), HorizontalAlignment = HorizontalAlignment.Center },
                new Rectangle { Width = 92, Height = 5, RadiusX = 2.5, RadiusY = 2.5, Fill = StandBrush(110) },
            },
        };

        Children.Add(_glow);
        Children.Add(stand);
        Children.Add(_screen);
        Children.Add(_chips);
        SizeChanged += (_, _) => Arrange();
    }

    public string Caption
    {
        get => _caption.Text;
        set => _caption.Text = value;
    }

    private static SolidColorBrush StandBrush(byte alpha) => new(Color.FromArgb(alpha, 128, 128, 140));

    public void SetLayout(StripLayout layout)
    {
        var slots = layout.Slots();
        if (slots.SequenceEqual(_slots))
        {
            return;
        }
        _slots = slots;
        _glow.Children.Clear();
        _chips.Children.Clear();
        _leds.Clear();
        foreach (var _ in _slots)
        {
            var inner = new GradientStop { Offset = 0, Color = Colors(0, 0, 0, 0) };
            var outer = new GradientStop { Offset = 1, Color = Colors(0, 0, 0, 0) };
            var glow = new Ellipse
            {
                Width = GlowSize,
                Height = GlowSize,
                Fill = new RadialGradientBrush { GradientStops = { inner, outer } },
            };
            var chipBrush = new SolidColorBrush(Colors(255, 0, 0, 0));
            var chip = new Ellipse { Width = 4, Height = 4, Fill = chipBrush };
            _glow.Children.Add(glow);
            _chips.Children.Add(chip);
            _leds.Add((glow, inner, outer, chip, chipBrush));
        }
        Arrange();
    }

    /// <summary>Flat RGB, one triplet per LED in strip order.</summary>
    public void SetColors(ReadOnlySpan<byte> rgb)
    {
        for (var i = 0; i < _leds.Count && i * 3 + 2 < rgb.Length; i++)
        {
            byte r = rgb[i * 3], g = rgb[i * 3 + 1], b = rgb[i * 3 + 2];
            var led = _leds[i];
            // Light, not paint: a dark LED should glow less, never cast a black smudge.
            var glow = (byte)(GlowAlpha * Math.Max(r, Math.Max(g, b)) / 255);
            led.Inner.Color = Colors(glow, r, g, b);
            led.Outer.Color = Colors(0, r, g, b);
            led.ChipBrush.Color = Colors(255, r, g, b);
        }
    }

    private static Color Colors(byte a, byte r, byte g, byte b) => Color.FromArgb(a, r, g, b);

    private void Arrange()
    {
        if (ActualWidth <= 0)
        {
            return;
        }
        // Mirrors the layout of _screen: centred, lifted by half its bottom margin.
        var left = (ActualWidth - ScreenWidth) / 2;
        var top = (ActualHeight - ScreenHeight - 22) / 2;
        for (var i = 0; i < _leds.Count && i < _slots.Count; i++)
        {
            var slot = _slots[i];
            var (cx, cy) = slot.Center;
            var (nx, ny) = slot.Side switch
            {
                Side.Left => (-1.0, 0.0),
                Side.Top => (0.0, -1.0),
                Side.Right => (1.0, 0.0),
                _ => (0.0, 1.0),
            };
            var x = left + cx * ScreenWidth;
            var y = top + cy * ScreenHeight;
            var led = _leds[i];
            Canvas.SetLeft(led.Glow, x + nx * GlowOffset - GlowSize / 2);
            Canvas.SetTop(led.Glow, y + ny * GlowOffset - GlowSize / 2);
            Canvas.SetLeft(led.Chip, x + nx * 3.5 - 2);
            Canvas.SetTop(led.Chip, y + ny * 3.5 - 2);
        }
    }
}
