using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace HaloSettings.Controls;

/// <summary>
/// Turns a StackPanel of SettingsCards into one block, the way Windows Settings groups
/// "Bluetooth devices": a single rounded container with hairline dividers between rows.
/// </summary>
public static class CardGroup
{
    private const double Radius = 8;

    public static readonly DependencyProperty IsGroupProperty = DependencyProperty.RegisterAttached(
        "IsGroup", typeof(bool), typeof(CardGroup), new PropertyMetadata(false, OnIsGroupChanged));

    public static bool GetIsGroup(DependencyObject element) => (bool)element.GetValue(IsGroupProperty);

    public static void SetIsGroup(DependencyObject element, bool value) => element.SetValue(IsGroupProperty, value);

    private static void OnIsGroupChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is StackPanel panel && e.NewValue is true)
        {
            panel.Loaded += (_, _) => Apply(panel);
        }
    }

    private static Brush Resource(string key) => (Brush)Application.Current.Resources[key];

    /// <summary>Restyles the panel's visible rows; call again after adding or removing rows.</summary>
    public static void Apply(StackPanel panel)
    {
        panel.Spacing = 0;
        panel.CornerRadius = new CornerRadius(Radius);
        panel.BorderThickness = new Thickness(1);
        panel.BorderBrush = Resource("CardStrokeColorDefaultBrush");
        panel.Background = Resource("CardBackgroundFillColorDefaultBrush");

        var rows = panel.Children.OfType<Control>().Where(c => c.Visibility == Visibility.Visible).ToList();
        var inner = Radius - 1;
        for (var i = 0; i < rows.Count; i++)
        {
            bool first = i == 0, last = i == rows.Count - 1;
            var row = rows[i];
            row.Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
            row.CornerRadius = new CornerRadius(first ? inner : 0, first ? inner : 0, last ? inner : 0, last ? inner : 0);
            row.BorderThickness = new Thickness(0, 0, 0, last ? 0 : 1);
            row.BorderBrush = Resource("DividerStrokeColorDefaultBrush");
        }
    }
}
