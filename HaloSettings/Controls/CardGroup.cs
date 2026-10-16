using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace HaloSettings.Controls;

/// <summary>
/// Turns a StackPanel of rows into one block, the way Windows Settings groups
/// "Bluetooth devices": a single rounded container with hairline dividers between rows.
/// Rows are SettingsCards or other controls; a plain Border row works as an in-block header.
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

        var rows = panel.Children.OfType<FrameworkElement>().Where(c => c.Visibility == Visibility.Visible).ToList();
        var inner = Radius - 1;
        for (var i = 0; i < rows.Count; i++)
        {
            bool first = i == 0, last = i == rows.Count - 1;
            var corners = new CornerRadius(first ? inner : 0, first ? inner : 0, last ? inner : 0, last ? inner : 0);
            var divider = new Thickness(0, 0, 0, last ? 0 : 1);
            var transparent = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
            switch (rows[i])
            {
                case Control control:
                    control.Background = transparent;
                    control.CornerRadius = corners;
                    control.BorderThickness = divider;
                    control.BorderBrush = Resource("DividerStrokeColorDefaultBrush");
                    break;
                case Border border:
                    border.CornerRadius = corners;
                    border.BorderThickness = divider;
                    border.BorderBrush = Resource("DividerStrokeColorDefaultBrush");
                    break;
            }
        }
    }
}
