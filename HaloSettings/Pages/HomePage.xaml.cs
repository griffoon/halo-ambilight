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
                break;
            case nameof(MainViewModel.ModeIndex):
                SyncMode();
                break;
        }
    }

    private void OnModeSelected(SelectorBar sender, SelectorBarSelectionChangedEventArgs args)
    {
        if (!_syncingMode && sender.SelectedItem is not null)
        {
            ViewModel.ModeIndex = sender.Items.IndexOf(sender.SelectedItem);
        }
    }

    private void OnModeSettingsClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e) =>
        App.MainWindow.NavigateTo(ViewModel.IsScreen ? "screen" : "effects");

    private void SyncMode()
    {
        var index = Math.Clamp(ViewModel.ModeIndex, 0, ModeBar.Items.Count - 1);
        if (ModeBar.SelectedItem == ModeBar.Items[index])
        {
            return;
        }
        _syncingMode = true;
        ModeBar.SelectedItem = ModeBar.Items[index];
        _syncingMode = false;
    }
}
