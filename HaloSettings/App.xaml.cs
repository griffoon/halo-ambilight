using Microsoft.UI.Xaml;

namespace HaloSettings;

public partial class App : Application
{
    private const string InstanceMutexName = @"Local\HaloWindowMutex";

    // Held for the life of the process so a second launch can tell a window is already open.
    private static Mutex? _instance;

    private static readonly string LogPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Halo", "window.log");

    public App()
    {
        InitializeComponent();
        UnhandledException += (_, e) => Log(e.Exception);
    }

    /// <summary>Appends a crash report next to the service log so failures are visible without a debugger.</summary>
    public static void Log(Exception e)
    {
        try
        {
            File.AppendAllText(LogPath, $"[{DateTime.Now:O}] {e}\n\n");
        }
        catch (IOException)
        {
            // Nowhere left to report to.
        }
    }

    public static MainWindow MainWindow { get; private set; } = null!;

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _instance = new Mutex(true, InstanceMutexName, out var created);
        if (!created)
        {
            // The tray service focuses the existing window by its title; nothing to do here.
            Exit();
            return;
        }
        try
        {
            MainWindow = new MainWindow();
            MainWindow.Activate();
        }
        catch (Exception e)
        {
            Log(e);
            throw;
        }
    }
}
