using System.ComponentModel;
using HaloSettings.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace HaloSettings.Pages;

public sealed partial class HomePage : HaloPage
{
    private bool _syncingMode;

    public HomePage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ViewModel.LedsUpdated += OnLeds;
        ViewModel.PropertyChanged += OnViewModelChanged;
        Preview.SetLayout(ViewModel.Layout);
        // Static modes send colours only when they change, so start from the last frame seen.
        Preview.SetColors(ViewModel.LastLeds);
        SyncMode();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        ViewModel.LedsUpdated -= OnLeds;
        ViewModel.PropertyChanged -= OnViewModelChanged;
        base.OnNavigatedFrom(e);
    }

    private void OnLeds(byte[] leds) => Preview.SetColors(leds);

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(MainViewModel.Layout):
                Preview.SetLayout(ViewModel.Layout);
                Preview.SetColors(ViewModel.LastLeds);
                break;
            case nameof(MainViewModel.ModeIndex):
                SyncMode();
                break;
        }
    }

    private RadioButton[] ModeButtons => [Mode0, Mode1, Mode2, Mode3, Mode4];

    private void OnModeChecked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (!_syncingMode && sender is RadioButton { Tag: string tag } && int.TryParse(tag, out var index))
        {
            ViewModel.ModeIndex = index;
        }
    }

    private void OnModeSettingsClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e) =>
        App.MainWindow.NavigateTo(ViewModel.IsScreen ? "screen" : "effects");

    private void SyncMode()
    {
        var button = ModeButtons[Math.Clamp(ViewModel.ModeIndex, 0, ModeButtons.Length - 1)];
        if (button.IsChecked == true)
        {
            return;
        }
        _syncingMode = true;
        button.IsChecked = true;
        _syncingMode = false;
    }
}
