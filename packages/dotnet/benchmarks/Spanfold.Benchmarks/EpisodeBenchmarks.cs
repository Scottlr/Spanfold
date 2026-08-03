using BenchmarkDotNet.Attributes;
using Spanfold;
using Spanfold.Episodes;

namespace Spanfold.Benchmarks;

[MemoryDiagnoser]
public class EpisodeFormationBenchmarks
{
    private EpisodeFormationBuilder formation = null!;

    [Params(128, 1_024, 8_192)]
    public int WindowCount { get; set; }

    [GlobalSetup]
    public void GlobalSetup()
    {
        var history = EpisodeBenchmarkData.CreateFormationHistory(WindowCount);
        this.formation = history.FormEpisodes("Benchmark formation")
            .From(selector => selector.Source(EpisodeBenchmarkData.TargetSource))
            .Within(scope => scope.Window(EpisodeBenchmarkData.WindowName))
            .StitchGapsUpTo(EpisodeBenchmarkData.StitchTolerance);

        _ = this.formation.Build();
    }

    [Benchmark]
    public EpisodeSet FormEpisodes()
    {
        return this.formation.Run();
    }
}

[MemoryDiagnoser]
public class EpisodeRelationBenchmarks
{
    private EpisodeComparisonBuilder comparison = null!;

    [Params(64, 256, 1_024)]
    public int EpisodeCountPerSide { get; set; }

    [GlobalSetup]
    public void GlobalSetup()
    {
        var history = EpisodeBenchmarkData.CreateRelationHistory(EpisodeCountPerSide);
        this.comparison = EpisodeBenchmarkData.CreateComparison(history);

        _ = this.comparison.Build();
    }

    [Benchmark]
    public EpisodeComparisonResult BuildSparseRelationGraph()
    {
        return this.comparison.Run();
    }
}

[MemoryDiagnoser]
public class EpisodeSummaryBenchmarks
{
    private EpisodeComparisonResult materializedComparison = null!;

    [GlobalSetup]
    public void GlobalSetup()
    {
        var history = EpisodeBenchmarkData.CreateRelationHistory(1_024);
        this.materializedComparison = EpisodeBenchmarkData.CreateComparison(history).Run();
    }

    [Benchmark]
    public EpisodeReferenceScorecard InterpretMaterializedReferenceScorecard()
    {
        return this.materializedComparison.AsReference();
    }
}

internal static class EpisodeBenchmarkData
{
    internal const string DetectionSource = "detection";
    internal const long StitchTolerance = 6;
    internal const string TargetSource = "reference";
    internal const string WindowName = "State";

    private const int FragmentsPerEpisode = 8;
    private const long RelationStride = 20;

    internal static WindowHistory CreateFormationHistory(int windowCount)
    {
        var windows = new ClosedWindow[windowCount];
        for (var index = 0; index < windowCount; index++)
        {
            var episodeIndex = index / FragmentsPerEpisode;
            var fragmentIndex = index % FragmentsPerEpisode;
            var start = fragmentIndex * 10L;
            windows[index] = Closed(
                "device-" + episodeIndex.ToString(System.Globalization.CultureInfo.InvariantCulture),
                start,
                start + 4,
                TargetSource);
        }

        return WindowHistory.FromRecords(windows, []);
    }

    internal static WindowHistory CreateRelationHistory(int episodeCountPerSide)
    {
        var windows = new ClosedWindow[episodeCountPerSide * 2];
        for (var episodeIndex = 0; episodeIndex < episodeCountPerSide; episodeIndex++)
        {
            var key = "device-" + episodeIndex.ToString(System.Globalization.CultureInfo.InvariantCulture);
            var targetStart = episodeIndex * RelationStride;
            windows[episodeIndex * 2] = Closed(key, targetStart, targetStart + 4, TargetSource);

            var detectionStart = episodeIndex % 4 == 3
                ? targetStart + 10
                : targetStart + 1;
            windows[(episodeIndex * 2) + 1] = Closed(
                key,
                detectionStart,
                detectionStart + 4,
                DetectionSource);
        }

        return WindowHistory.FromRecords(windows, []);
    }

    internal static EpisodeComparisonBuilder CreateComparison(WindowHistory history)
    {
        return history.CompareEpisodes("Benchmark detector evaluation")
            .Target(TargetSource, selector => selector.Source(TargetSource))
            .Against(DetectionSource, selector => selector.Source(DetectionSource))
            .Within(scope => scope.Window(WindowName));
    }

    private static ClosedWindow Closed(string key, long start, long end, string source)
    {
        return new ClosedWindow(WindowName, key, start, end, Source: source);
    }
}
