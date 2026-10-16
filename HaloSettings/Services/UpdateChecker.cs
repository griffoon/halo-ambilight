using System.Diagnostics;
using System.Net.Http.Headers;
using System.Reflection;
using System.Text.Json.Nodes;

namespace HaloSettings.Services;

public sealed record UpdateInfo(Version Version, string Url);

/// <summary>
/// Asks GitHub for the latest Halo release. Only reads public release metadata: nothing about
/// this machine or its settings is sent.
/// </summary>
public static class UpdateChecker
{
    public const string RepoUrl = "https://github.com/griffoon/halo-ambilight";
    public const string IssuesUrl = RepoUrl + "/issues";
    private const string LatestApi = "https://api.github.com/repos/griffoon/halo-ambilight/releases/latest";

    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(10);

    public static Version Current { get; } =
        Assembly.GetExecutingAssembly().GetName().Version is { } v ? new Version(v.Major, v.Minor, Math.Max(v.Build, 0)) : new Version(0, 0, 0);

    /// <summary>The newer release, or null when this build is already the latest.</summary>
    public static async Task<UpdateInfo?> CheckAsync(CancellationToken ct)
    {
        using var http = new HttpClient { Timeout = Timeout };
        http.DefaultRequestHeaders.UserAgent.Add(new ProductInfoHeaderValue("Halo", Current.ToString()));
        http.DefaultRequestHeaders.Accept.Add(new MediaTypeWithQualityHeaderValue("application/vnd.github+json"));

        var json = JsonNode.Parse(await http.GetStringAsync(LatestApi, ct));
        var tag = json?["tag_name"]?.GetValue<string>();
        var url = json?["html_url"]?.GetValue<string>();
        if (ParseTag(tag) is not { } latest || url is null || !url.StartsWith(RepoUrl, StringComparison.Ordinal))
        {
            throw new InvalidDataException("unexpected release data");
        }
        return latest > Current ? new UpdateInfo(latest, url) : null;
    }

    /// <summary>"v0.9.0" or "0.9.0" to a version; anything else, such as a pre-release tag, is null.</summary>
    public static Version? ParseTag(string? tag)
    {
        var text = tag?.Trim().TrimStart('v', 'V');
        return Version.TryParse(text, out var v) && v.Major >= 0 ? new Version(v.Major, v.Minor, Math.Max(v.Build, 0)) : null;
    }

    public static void Open(string url)
    {
        // Only our own GitHub pages are ever opened from here.
        if (url.StartsWith(RepoUrl, StringComparison.Ordinal))
        {
            Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
        }
    }
}
