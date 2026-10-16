using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace HaloSettings.Controls;

/// <summary>
/// A standard Slider with a small mark under the track where the factory default sits.
/// Double-click resets to the default.
/// </summary>
public sealed partial class DefaultSlider : Grid
{
    // Fluent's horizontal thumb is 18 px; its centre travels from 9 px to width - 9 px.
    private const double ThumbWidth = 18;
    private const double MarkSize = 4;
    private const double MarkOffset = 13;

    public static readonly DependencyProperty ValueProperty = DependencyProperty.Register(
        nameof(Value), typeof(double), typeof(DefaultSlider),
        new PropertyMetadata(0.0, (d, e) => ((DefaultSlider)d)._slider.Value = (double)e.NewValue));

    private readonly Slider _slider = new() { VerticalAlignment = VerticalAlignment.Center };
    private readonly Ellipse _mark = new()
    {
        Width = MarkSize,
        Height = MarkSize,
        HorizontalAlignment = HorizontalAlignment.Left,
        VerticalAlignment = VerticalAlignment.Center,
        IsHitTestVisible = false,
    };

    public DefaultSlider()
    {
        Width = 200;
        _mark.Fill = (Brush)Application.Current.Resources["TextFillColorTertiaryBrush"];
        Children.Add(_mark);
        Children.Add(_slider);
        _slider.ValueChanged += (_, e) => Value = e.NewValue;
        _slider.DoubleTapped += (_, _) => Value = DefaultValue;
        SizeChanged += (_, _) => PlaceMark();
        Loaded += (_, _) =>
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

    private void PlaceMark()
    {
        var span = _slider.Maximum - _slider.Minimum;
        if (ActualWidth <= ThumbWidth || span <= 0)
        {
            return;
        }
        var t = Math.Clamp((DefaultValue - _slider.Minimum) / span, 0, 1);
        var x = ThumbWidth / 2 + (ActualWidth - ThumbWidth) * t - MarkSize / 2;
        _mark.Margin = new Thickness(x, MarkOffset * 2, 0, 0);
    }
}
