using HaloSettings.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace HaloSettings.Pages;

/// <summary>
/// Every page shares the window's single view model, handed over as the navigation parameter.
/// OnNavigatedTo runs before x:Bind initialises, so bindings always see it.
/// </summary>
public partial class HaloPage : Page
{
    public MainViewModel ViewModel { get; private set; } = null!;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        ViewModel = (MainViewModel)e.Parameter;
        base.OnNavigatedTo(e);
    }
}
