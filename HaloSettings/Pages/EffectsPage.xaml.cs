using System.ComponentModel;
using HaloSettings.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;
using Windows.UI;

namespace HaloSettings.Pages;

public sealed partial class EffectsPage : HaloPage
{
    private static readonly (string Name, Color Color)[] Presets =
    [
        ("Тёплый белый", Color.FromArgb(255, 255, 170, 90)),
        ("Белый", Color.FromArgb(255, 255, 236, 214)),
        ("Красный", Color.FromArgb(255, 255, 44, 32)),
        ("Оранжевый", Color.FromArgb(255, 255, 112, 24)),
        ("Жёлтый", Color.FromArgb(255, 255, 204, 40)),
        ("Зелёный", Color.FromArgb(255, 60, 230, 96)),
        ("Голубой", Color.FromArgb(255, 40, 212, 255)),
        ("Синий", Color.FromArgb(255, 52, 92, 255)),
        ("Фиолетовый", Color.FromArgb(255, 150, 72, 255)),
        ("Розовый", Color.FromArgb(255, 255, 62, 170)),
    ];

    private readonly List<(Button Button, Color Color)> _swatches = [];

    public EffectsPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (_swatches.Count == 0)
        {
            BuildSwatches();
        }
        ViewModel.PropertyChanged += OnViewModelChanged;
        SyncSwatches();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        ViewModel.PropertyChanged -= OnViewModelChanged;
        base.OnNavigatedFrom(e);
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(MainViewModel.Color))
        {
            SyncSwatches();
        }
    }

    private void BuildSwatches()
    {
        foreach (var (name, color) in Presets)
        {
            var button = SwatchButton(new Ellipse { Width = 20, Height = 20, Fill = new SolidColorBrush(color) }, name);
            button.Click += (_, _) => ViewModel.Color = color;
            _swatches.Add((button, color));
            Swatches.Children.Add(button);
        }

        var picker = new ColorPicker
        {
            IsAlphaEnabled = false,
            IsColorChannelTextInputVisible = false,
            IsHexInputVisible = true,
            ColorSpectrumShape = ColorSpectrumShape.Ring,
        };
        picker.ColorChanged += (_, args) => ViewModel.Color = args.NewColor;
        var custom = SwatchButton(new FontIcon { Glyph = "", FontSize = 12 }, "Свой цвет");
        custom.Flyout = new Flyout { Content = picker };
        custom.Click += (_, _) => picker.Color = ViewModel.Color;
        Swatches.Children.Add(custom);
    }

    private static Button SwatchButton(UIElement content, string name)
    {
        var button = new Button
        {
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(16),
            Content = content,
        };
        AutomationProperties.SetName(button, name);
        ToolTipService.SetToolTip(button, name);
        return button;
    }

    private void SyncSwatches()
    {
        var accent = (Brush)Application.Current.Resources["AccentFillColorDefaultBrush"];
        var idle = (Brush)Application.Current.Resources["ControlStrokeColorDefaultBrush"];
        foreach (var (button, color) in _swatches)
        {
            var selected = color == ViewModel.Color;
            button.BorderBrush = selected ? accent : idle;
            button.BorderThickness = new Thickness(selected ? 2 : 1);
        }
    }
}
