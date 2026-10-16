using HaloSettings.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Windows.UI;

namespace HaloSettings.Controls;

/// <summary>
/// A monitor drawn at the centre with the strip's live colours as a soft blur around it.
/// Each LED is a wide radial glow; neighbours overlap into one continuous halo.
/// The screen itself is always dark: it is a picture of a monitor, not app chrome.
/// </summary>
public sealed partial class LedPreview : Grid
{
    private const double ScreenWidth = 236;
    private const double ScreenHeight = 134;
    private const double ScreenLift = 22;
    private const double GlowSize = 120;
    private const double GlowOffset = 20;
    private const byte GlowAlpha = 110;

    private readonly Canvas _glow = new() { IsHitTestVisible = false };
    private readonly TextBlock _caption;
    private readonly List<(Ellipse Glow, GradientStop Inner, GradientStop Outer)> _leds = [];
    private IReadOnlyList<Slot> _slots = [];

    public LedPreview()
    {
        Height = 220;
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
        _glow.Children.Clear();
        _leds.Clear();
        foreach (var _ in _slots)
        {
            var inner = new GradientStop { Offset = 0, Color = Color.FromArgb(0, 0, 0, 0) };
            var outer = new GradientStop { Offset = 1, Color = Color.FromArgb(0, 0, 0, 0) };
            var glow = new Ellipse
            {
                Width = GlowSize,
                Height = GlowSize,
                Fill = new RadialGradientBrush { GradientStops = { inner, outer } },
            };
            _glow.Children.Add(glow);
            _leds.Add((glow, inner, outer));
        }
        Arrange();
    }

    /// <summary>Flat RGB, one triplet per LED in strip order.</summary>
    public void SetColors(ReadOnlySpan<byte> rgb)
    {
        for (var i = 0; i < _leds.Count && i * 3 + 2 < rgb.Length; i++)
        {
            byte r = rgb[i * 3], g = rgb[i * 3 + 1], b = rgb[i * 3 + 2];
            // Light, not paint: a dark LED glows less and an unlit one not at all, never a black smudge.
            var alpha = (byte)(GlowAlpha * Math.Max(r, Math.Max(g, b)) / 255);
            var led = _leds[i];
            led.Inner.Color = Color.FromArgb(alpha, r, g, b);
            led.Outer.Color = Color.FromArgb(0, r, g, b);
        }
    }

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
            var glow = _leds[i].Glow;
            Canvas.SetLeft(glow, left + cx * ScreenWidth + nx * GlowOffset - GlowSize / 2);
            Canvas.SetTop(glow, top + cy * ScreenHeight + ny * GlowOffset - GlowSize / 2);
        }
    }
}
