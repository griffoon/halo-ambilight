using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;

namespace HaloSettings.Controls;

/// <summary>
/// A standard Slider with a small mark under the track where the factory default sits.
/// The mark is placed from the slider's own template parts, so it lines up with the thumb exactly.
/// Double-click resets to the default.
/// </summary>
public sealed partial class DefaultSlider : Grid
{
    private const double MarkSize = 4;
    private const double MarkGap = 5;

    public static readonly DependencyProperty ValueProperty = DependencyProperty.Register(
        nameof(Value), typeof(double), typeof(DefaultSlider),
        new PropertyMetadata(0.0, (d, e) => ((DefaultSlider)d)._slider.Value = (double)e.NewValue));

    private readonly Slider _slider = new() { VerticalAlignment = VerticalAlignment.Center };
    private readonly Ellipse _mark = new()
    {
        Width = MarkSize,
        Height = MarkSize,
        HorizontalAlignment = HorizontalAlignment.Left,
        VerticalAlignment = VerticalAlignment.Top,
        IsHitTestVisible = false,
        Opacity = 0,
    };

    public DefaultSlider()
    {
        Width = 200;
        _mark.Fill = (Brush)Application.Current.Resources["TextFillColorTertiaryBrush"];
        Children.Add(_mark);
        Children.Add(_slider);
        _slider.ValueChanged += (_, e) => Value = e.NewValue;
        _slider.DoubleTapped += (_, _) => Value = DefaultValue;
        _slider.SizeChanged += (_, _) => PlaceMark();
        _slider.Loaded += (_, _) =>
        {
            AutomationProperties.SetHelpText(_slider, $"Двойной щелчок возвращает значение по умолчанию: {DefaultValue:0.#}");
            PlaceMark();
        };
    }

    public double Value
    {
        get => (double)GetValue(ValueProperty);
        set => SetValue(ValueProperty, value);
    }

    public double Minimum
    {
        get => _slider.Minimum;
        set => _slider.Minimum = value;
    }

    public double Maximum
    {
        get => _slider.Maximum;
        set => _slider.Maximum = value;
    }

    public double StepFrequency
    {
        get => _slider.StepFrequency;
        set => _slider.StepFrequency = value;
    }

    public double DefaultValue { get; set; }

    private static FrameworkElement? FindPart(DependencyObject root, string name)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is FrameworkElement fe && fe.Name == name)
            {
                return fe;
            }
            if (FindPart(child, name) is { } found)
            {
                return found;
            }
        }
        return null;
    }

    private void PlaceMark()
    {
        var track = FindPart(_slider, "HorizontalTrackRect");
        var thumb = FindPart(_slider, "HorizontalThumb");
        var span = _slider.Maximum - _slider.Minimum;
        if (track is null || thumb is null || track.ActualWidth <= 0 || thumb.ActualWidth <= 0 || span <= 0)
        {
            return;
        }
        // The thumb's centre travels from half a thumb in from the left of the track to half a thumb in from the right.
        var origin = track.TransformToVisual(this).TransformPoint(new Point(0, 0));
        var t = Math.Clamp((DefaultValue - _slider.Minimum) / span, 0, 1);
        var x = origin.X + thumb.ActualWidth / 2 + (track.ActualWidth - thumb.ActualWidth) * t;
        var y = origin.Y + track.ActualHeight / 2 + thumb.ActualHeight / 2 + MarkGap;
        _mark.Margin = new Thickness(x - MarkSize / 2, y - MarkSize / 2, 0, 0);
        _mark.Opacity = 1;
    }
}
