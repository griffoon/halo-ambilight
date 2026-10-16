using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using HaloSettings.Services;

namespace HaloSettings.ViewModels;

/// <summary>Update check against GitHub releases: once when the window opens, and on request.</summary>
public sealed partial class MainViewModel
{
    public string VersionTitle { get; } = $"Halo {UpdateChecker.Current}";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(UpdateTitle))]
    private UpdateInfo? _update;

    /// <summary>The banner on the home page; closing it hides it until the window is reopened.</summary>
    [ObservableProperty]
    private bool _updateOpen;

    [ObservableProperty]
    private string _updateStatus = "Подсветка за монитором для лент Skydimo";

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(CheckForUpdatesCommand))]
    private bool _isCheckingUpdates;

    public string UpdateTitle => Update is { } u ? $"Доступна версия {u.Version}" : "";

    private bool CanCheckForUpdates => !IsCheckingUpdates;

    [RelayCommand(CanExecute = nameof(CanCheckForUpdates))]
    private Task CheckForUpdates() => CheckForUpdatesAsync(silent: false);

    [RelayCommand]
    private void DownloadUpdate() => UpdateChecker.Open(Update?.Url ?? UpdateChecker.RepoUrl + "/releases/latest");

    [RelayCommand]
    private void OpenRepo() => UpdateChecker.Open(UpdateChecker.RepoUrl);

    [RelayCommand]
    private void ReportProblem() => UpdateChecker.Open(UpdateChecker.IssuesUrl);

    /// <summary>A silent check stays quiet when offline; a manual one says what happened.</summary>
    private async Task CheckForUpdatesAsync(bool silent)
    {
        IsCheckingUpdates = true;
        if (!silent)
        {
            UpdateStatus = "Проверяем обновления";
        }
        try
        {
            Update = await UpdateChecker.CheckAsync(CancellationToken.None);
            UpdateOpen = Update is not null;
            UpdateStatus = Update is { } u ? $"Доступна версия {u.Version}" : "Установлена последняя версия";
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or InvalidDataException or System.Text.Json.JsonException)
        {
            if (!silent)
            {
                UpdateStatus = "Не удалось проверить, нет связи с GitHub";
            }
        }
        finally
        {
            IsCheckingUpdates = false;
        }
    }
}
