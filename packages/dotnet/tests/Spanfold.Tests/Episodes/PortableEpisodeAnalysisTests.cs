using System.Text.Json;

using Spanfold.Artifacts.Episodes;

namespace Spanfold.Tests.Episodes;

public sealed class PortableEpisodeAnalysisTests
{
    [Fact]
    public void SharedProviderDetectorFixtureMatchesPortableResultContract()
    {
        var fixtureDirectory = FindFixtureDirectory();
        var document = EpisodeAnalysisDocument.Read(
            Path.Combine(fixtureDirectory, "portable-provider-detector-plan.json"));
        var history = ReadHistory(
            Path.Combine(fixtureDirectory, "portable-provider-detector-windows.jsonl"));

        var result = document.Execute(history);

        var expected = File.ReadAllText(
            Path.Combine(fixtureDirectory, "portable-provider-detector-result.json"));
        Assert.Equal(expected.Trim(), result.ExportJson().Trim());
        var markdown = result.ExportMarkdown();
        Assert.Contains("## Target episodes: provider", markdown, StringComparison.Ordinal);
        Assert.Contains("| oneToOne | 1 | 1 | provisional |", markdown, StringComparison.Ordinal);
        Assert.DoesNotContain("episodeId", markdown, StringComparison.Ordinal);
    }

    [Fact]
    public void VersionOneRejectsTimestampWithoutClockContract()
    {
        var json = """
            {
              "schema": "spanfold.episode.analysis",
              "schemaVersion": 1,
              "name": "comparison",
              "target": { "name": "provider", "source": "provider-a" },
              "against": { "name": "detector", "source": "detector-b" },
              "windowName": "Offline",
              "normalizationAxis": "timestamp",
              "stitchTolerance": 0,
              "relationTolerance": 0
            }
            """;

        var exception = Assert.Throws<ArgumentException>(() => EpisodeAnalysisDocument.ParseJson(json));

        Assert.Contains("supports only the 'processingPosition'", exception.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void VersionOneRejectsNegativeLiveHorizon()
    {
        var json = """
            {
              "schema": "spanfold.episode.analysis",
              "schemaVersion": 1,
              "name": "negative horizon",
              "target": { "name": "provider", "source": "provider-a" },
              "against": { "name": "detector", "source": "detector-b" },
              "windowName": "Offline",
              "normalizationAxis": "processingPosition",
              "stitchTolerance": 0,
              "relationTolerance": 0,
              "liveHorizon": -1
            }
            """;

        var exception = Assert.Throws<ArgumentException>(() => EpisodeAnalysisDocument.ParseJson(json));

        Assert.Equal("$.liveHorizon must be a non-negative integer or null.", exception.Message);
    }

    [Fact]
    public void VersionOneRejectsNonStringIdentity()
    {
        var fixtureDirectory = FindFixtureDirectory();
        var document = EpisodeAnalysisDocument.Read(
            Path.Combine(fixtureDirectory, "portable-provider-detector-plan.json"));
        var numericKeyHistory = WindowHistory.FromRecords([
            new ClosedWindow("DeviceOffline", 42, 1, 4, "provider-a", true),
            new ClosedWindow("DeviceOffline", 42, 1, 4, "detector-b", true)
        ], []);

        var keyException = Assert.Throws<InvalidOperationException>(() => document.Execute(numericKeyHistory));

        Assert.Equal(
            "Portable Episode analysis requires string keys and string-or-null partitions.",
            keyException.Message);
        var booleanPartitionHistory = WindowHistory.FromRecords([
            new ClosedWindow("DeviceOffline", "device-1", 1, 4, "provider-a", true),
            new ClosedWindow("DeviceOffline", "device-1", 1, 4, "detector-b", true)
        ], []);
        var partitionException = Assert.Throws<InvalidOperationException>(
            () => document.Execute(booleanPartitionHistory));

        Assert.Equal(
            "Portable Episode analysis requires string keys and string-or-null partitions.",
            partitionException.Message);
    }

    [Fact]
    public void UnicodeIdentityUsesPortableUtf8OrderAndJsonMarkdownLiterals()
    {
        var fixtureDirectory = FindFixtureDirectory();
        var document = EpisodeAnalysisDocument.Read(
            Path.Combine(fixtureDirectory, "portable-provider-detector-plan.json"));
        var history = ReadHistory(
            Path.Combine(fixtureDirectory, "portable-unicode-order-windows.jsonl"));

        var result = document.Execute(history);
        var expected = File.ReadAllText(
            Path.Combine(fixtureDirectory, "portable-unicode-order-result.json"));
        var exported = result.ExportJson();

        Assert.Equal(expected.TrimEnd('\r', '\n'), exported);
        using var json = JsonDocument.Parse(exported);
        var episodes = json.RootElement.GetProperty("target").GetProperty("episodes");
        Assert.Equal("same", episodes[0].GetProperty("key").GetString());
        Assert.Equal(JsonValueKind.Null, episodes[0].GetProperty("partition").ValueKind);
        Assert.Equal("same", episodes[1].GetProperty("key").GetString());
        Assert.Equal("null", episodes[1].GetProperty("partition").GetString());
        Assert.Equal("", episodes[2].GetProperty("key").GetString());
        Assert.Equal("😀", episodes[3].GetProperty("key").GetString());
        var relations = json.RootElement.GetProperty("relations");
        Assert.Equal(0, relations[0].GetProperty("targetEpisodeIndexes")[0].GetInt32());
        Assert.Equal(3, relations[3].GetProperty("targetEpisodeIndexes")[0].GetInt32());

        var markdown = result.ExportMarkdown();
        Assert.Contains("| 0 | \"same\" | null |", markdown, StringComparison.Ordinal);
        Assert.Contains("| 1 | \"same\" | \"null\" |", markdown, StringComparison.Ordinal);
        Assert.Contains("| 2 | \"\" | null |", markdown, StringComparison.Ordinal);
        Assert.Contains("| 3 | \"😀\" | \"null\" |", markdown, StringComparison.Ordinal);
    }

    private static string FindFixtureDirectory()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null)
        {
            var candidate = Path.Combine(directory.FullName, "features", "episodes", "fixtures");
            if (Directory.Exists(candidate))
            {
                return candidate;
            }

            directory = directory.Parent;
        }

        throw new DirectoryNotFoundException("Could not find the shared Episode fixtures.");
    }

    private static WindowHistory ReadHistory(string path)
    {
        var closed = new List<ClosedWindow>();
        var open = new List<OpenWindow>();
        foreach (var line in File.ReadLines(path))
        {
            using var parsed = JsonDocument.Parse(line);
            var row = parsed.RootElement;
            var windowName = row.GetProperty("windowName").GetString()!;
            var key = row.GetProperty("key").GetString()!;
            var source = row.GetProperty("source").GetString();
            var partition = row.GetProperty("partition").ValueKind == JsonValueKind.Null
                ? null
                : row.GetProperty("partition").GetString();
            var start = row.GetProperty("startPosition").GetInt64();
            var end = row.GetProperty("endPosition");
            if (end.ValueKind == JsonValueKind.Null)
            {
                open.Add(new OpenWindow(windowName, key, start, source, partition));
            }
            else
            {
                closed.Add(new ClosedWindow(windowName, key, start, end.GetInt64(), source, partition));
            }
        }

        return WindowHistory.FromRecords(closed, open);
    }
}
