using System.Numerics;
using HaloSettings.Services;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Windows.UI;

namespace HaloSettings.Controls;

/// <summary>
/// A monitor drawn at the centre with the strip's live colours as a soft blur around it.
/// Each LED is a wide radial glow; neighbours overlap into one continuous halo.
/// The glows are composition visuals: a XAML RadialGradientBrush ignores colour changes to its stops
/// once it has rendered, which froze the preview on its first frame.
/// The screen itself is always dark: it is a picture of a monitor, not app chrome.
/// </summary>
public sealed partial class LedPreview : Grid
{
    private const double ScreenWidth = 236;
    private const double ScreenHeight = 134;
    private const double ScreenLift = 22;
    private const double GlowSize = 100;
    private const double GlowOffset = 16;
    private const byte GlowAlpha = 180;
    // Values this low are capture noise on a black frame; normalising them would paint speckles.
    private const byte NoiseFloor = 6;

    private readonly Canvas _glow = new() { IsHitTestVisible = false };
    private readonly TextBlock _caption;
    private readonly Compositor _compositor;
    private readonly ContainerVisual _glowRoot;
    private readonly List<(SpriteVisual Glow, CompositionColorGradientStop Inner, CompositionColorGradientStop Mid, CompositionColorGradientStop Outer)> _leds = [];
    private IReadOnlyList<Slot> _slots = [];
    private byte[] _shown = [];

    public LedPreview()
    {
        _compositor = ElementCompositionPreview.GetElementVisual(_glow).Compositor;
        _glowRoot = _compositor.CreateContainerVisual();
        ElementCompositionPreview.SetElementChildVisual(_glow, _glowRoot);
        _caption = new TextBlock
        {
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = new SolidColorBrush(Color.FromArgb(150, 255, 255, 255)),
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        };
        var screen = new Border
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
            Margin = new Thickness(0, 0, 0, ScreenLift),
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
        Children.Add(screen);
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
        _glowRoot.Children.RemoveAll();
        _leds.Clear();
        _shown = [];
        var clear = Color.FromArgb(0, 0, 0, 0);
        foreach (var _ in _slots)
        {
            var inner = _compositor.CreateColorGradientStop(0, clear);
            // A mid stop bends the falloff, so each glow has faded out before the edge of the control.
            var mid = _compositor.CreateColorGradientStop(0.45f, clear);
            var outer = _compositor.CreateColorGradientStop(1, clear);
            var brush = _compositor.CreateRadialGradientBrush();
            brush.ColorStops.Add(inner);
            brush.ColorStops.Add(mid);
            brush.ColorStops.Add(outer);
            var glow = _compositor.CreateSpriteVisual();
            glow.Size = new Vector2((float)GlowSize);
            glow.Brush = brush;
            _glowRoot.Children.InsertAtTop(glow);
            _leds.Add((glow, inner, mid, outer));
        }
        Arrange();
    }

    /// <summary>Flat RGB, one triplet per LED in strip order.</summary>
    public void SetColors(ReadOnlySpan<byte> rgb)
    {
        for (var i = 0; i < _leds.Count && i * 3 + 2 < rgb.Length; i++)
        {
            byte r = rgb[i * 3], g = rgb[i * 3 + 1], b = rgb[i * 3 + 2];
            // Skip unchanged LEDs: static modes resend the same frame.
            if (i * 3 + 2 < _shown.Length && _shown[i * 3] == r && _shown[i * 3 + 1] == g && _shown[i * 3 + 2] == b)
            {
                continue;
            }
            var led = _leds[i];
            var (alpha, hue) = Glow(r, g, b);
            led.Inner.Color = Color.FromArgb(alpha, hue.R, hue.G, hue.B);
            led.Mid.Color = Color.FromArgb((byte)(alpha / 4), hue.R, hue.G, hue.B);
            led.Outer.Color = Color.FromArgb(0, hue.R, hue.G, hue.B);
        }
        _shown = rgb.ToArray();
    }

    /// <summary>
    /// Light, not paint. Screen edges are usually dim, while the strip itself reads bright, so each
    /// channel is lifted on a square-root curve: hues keep their balance and greys stay grey, never
    /// turning white. Opacity follows the brightest channel on the same curve, so an unlit LED shows
    /// nothing at all, not a black smudge.
    /// </summary>
    private static (byte Alpha, Color Hue) Glow(byte r, byte g, byte b)
    {
        var peak = Math.Max(r, Math.Max(g, b));
        if (peak < NoiseFloor)
        {
            return (0, Color.FromArgb(0, 0, 0, 0));
        }
        var hue = Color.FromArgb(255, Lift(r), Lift(g), Lift(b));
        return ((byte)(GlowAlpha * Math.Sqrt(peak / 255.0)), hue);
    }

    private static byte Lift(byte v) => (byte)Math.Round(255 * Math.Sqrt(v / 255.0));

    private void Arrange()
    {
        if (ActualWidth <= 0)
        {
            return;
        }
        // Mirrors the screen's placement: centred, lifted by its bottom margin.
        var left = (ActualWidth - ScreenWidth) / 2;
        var top = (ActualHeight - ScreenHeight - ScreenLift) / 2;
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
            _leds[i].Glow.Offset = new Vector3(
                (float)(left + cx * ScreenWidth + nx * GlowOffset - GlowSize / 2),
                (float)(top + cy * ScreenHeight + ny * GlowOffset - GlowSize / 2),
                0);
        }
    }
}
