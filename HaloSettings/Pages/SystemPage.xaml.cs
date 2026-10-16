using HaloSettings.Services;
using Microsoft.UI.Xaml;

namespace HaloSettings.Pages;

public sealed partial class SystemPage : HaloPage
{
    public SystemPage()
    {
        InitializeComponent();
    }

    public Visibility HasUpdate(UpdateInfo? update) => update is null ? Visibility.Collapsed : Visibility.Visible;

    public Visibility NoUpdate(UpdateInfo? update) => update is null ? Visibility.Visible : Visibility.Collapsed;
}
