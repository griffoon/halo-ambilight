using System.Runtime.InteropServices;
using HaloSettings.Pages;
using HaloSettings.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using Windows.Graphics;

namespace HaloSettings;

public sealed partial class MainWindow : Window
{
    // Fixed size for now: single-purpose utility width, tall enough for the longest page.
    // Responsive layout is on the backlog.
    private const int WidthDip = 560;
    private const int HeightDip = 860;

    private static readonly Dictionary<string, Type> Pages = new()
    {
        ["home"] = typeof(HomePage),
        ["screen"] = typeof(ScreenPage),
        ["effects"] = typeof(EffectsPage),
        ["strip"] = typeof(StripPage),
        ["devices"] = typeof(DevicesPage),
        ["settings"] = typeof(SystemPage),
    };

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    public MainWindow()
    {
        InitializeComponent();
        ViewModel = new MainViewModel(DispatcherQueue);

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.SetIcon("Assets/AppIcon.ico");
        AppWindow.Title = "Halo";

        var hwnd = Win32Interop.GetWindowFromWindowId(AppWindow.Id);
        var scale = GetDpiForWindow(hwnd) / 96.0;
        AppWindow.Resize(new SizeInt32((int)(WidthDip * scale), (int)(HeightDip * scale)));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
        }
        CenterOnScreen();

        Closed += (_, _) => ViewModel.Shutdown();
        Nav.SelectedItem = Nav.MenuItems[0];
    }

    public MainViewModel ViewModel { get; }

    /// <summary>Opens a page by its navigation tag, as if picked from the menu.</summary>
    public void NavigateTo(string tag)
    {
        var item = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string)i.Tag == tag);
        if (item is not null)
        {
            Nav.SelectedItem = item;
        }
    }

    private void OnNavigationSelected(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        var tag = args.IsSettingsSelected ? "settings" : (args.SelectedItem as NavigationViewItem)?.Tag as string;
        if (tag is null || !Pages.TryGetValue(tag, out var page) || RootFrame.CurrentSourcePageType == page)
        {
            return;
        }
        RootFrame.Navigate(page, ViewModel, new EntranceNavigationTransitionInfo());
        Nav.IsPaneOpen = false;
    }

    private void OnNavigated(object sender, NavigationEventArgs e)
    {
        AppTitleBar.IsBackButtonEnabled = RootFrame.CanGoBack;
        var tag = Pages.FirstOrDefault(p => p.Value == e.SourcePageType).Key;
        if (tag == "settings")
        {
            Nav.SelectedItem = Nav.SettingsItem;
            return;
        }
        var item = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string)i.Tag == tag);
        if (item is not null && !ReferenceEquals(Nav.SelectedItem, item))
        {
            Nav.SelectedItem = item;
        }
    }

    private void OnBackRequested(TitleBar sender, object args)
    {
        if (RootFrame.CanGoBack)
        {
            RootFrame.GoBack();
        }
    }

    private void OnPaneToggleRequested(TitleBar sender, object args) => Nav.IsPaneOpen = !Nav.IsPaneOpen;

    private void CenterOnScreen()
    {
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var size = AppWindow.Size;
        AppWindow.Move(new PointInt32(area.X + (area.Width - size.Width) / 2, area.Y + (area.Height - size.Height) / 2));
    }
}
