using System.Collections.Specialized;
using CommunityToolkit.WinUI.Controls;
using HaloSettings.Controls;
using HaloSettings.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Navigation;

namespace HaloSettings.Pages;

/// <summary>The device list is rebuilt in code so it can share the grouped-card look.</summary>
public sealed partial class DevicesPage : HaloPage
{
    public DevicesPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ViewModel.OpenRgbDevices.CollectionChanged += OnDevicesChanged;
        Rebuild();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        ViewModel.OpenRgbDevices.CollectionChanged -= OnDevicesChanged;
        base.OnNavigatedFrom(e);
    }

    private void OnDevicesChanged(object? sender, NotifyCollectionChangedEventArgs e) => Rebuild();

    private void Rebuild()
    {
        DeviceList.Children.Clear();
        var any = ViewModel.OpenRgbDevices.Count > 0;
        DevicesHeader.Visibility = any ? Visibility.Visible : Visibility.Collapsed;
        DeviceList.Visibility = any ? Visibility.Visible : Visibility.Collapsed;
        foreach (var device in ViewModel.OpenRgbDevices)
        {
            var toggle = new ToggleSwitch();
            toggle.SetBinding(ToggleSwitch.IsOnProperty, new Binding
            {
                Source = device,
                Path = new PropertyPath(nameof(OpenRgbDevice.IsOn)),
                Mode = BindingMode.TwoWay,
            });
            DeviceList.Children.Add(new SettingsCard { Header = device.Name, Content = toggle });
        }
        if (any)
        {
            CardGroup.Apply(DeviceList);
        }
    }
}
