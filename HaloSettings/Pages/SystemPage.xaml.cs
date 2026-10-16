using System.Reflection;

namespace HaloSettings.Pages;

public sealed partial class SystemPage : HaloPage
{
    public SystemPage()
    {
        InitializeComponent();
    }

    public string Version { get; } =
        $"Halo {Assembly.GetExecutingAssembly().GetName().Version?.ToString(3) ?? ""}";
}
