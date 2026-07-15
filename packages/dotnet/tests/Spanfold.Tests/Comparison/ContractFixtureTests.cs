using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.Json;

using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ContractFixtureTests
{
    [Fact]
    public void ContractFixturesExecutePlans()
    {
        foreach (var fixturePath in FixturePaths())
        {
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            var result = ExecuteFixture(fixture.RootElement);

            Assert.NotNull(result);
            Assert.Equal(
                fixture.RootElement.GetProperty("expected").GetProperty("isValid").GetBoolean(),
                result.IsValid);
        }
    }

    [Fact]
    public void FixtureResultsMatchExpectedJson()
    {
        foreach (var fixturePath in FixturePaths())
        {
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            using var exported = JsonDocument.Parse(ExecuteFixture(fixture.RootElement).ExportJson());

            AssertExpectedDiagnostics(fixture.RootElement, exported.RootElement);
            AssertExpectedSummaries(fixture.RootElement, exported.RootElement);
            AssertExpectedRows(fixture.RootElement, exported.RootElement);
        }
    }

    [Fact]
    public void InvalidFixturePlansProduceExpectedDiagnostics()
    {
        var invalidFixturePaths = FixturePaths()
            .Where(path =>
            {
                using var fixture = JsonDocument.Parse(File.ReadAllText(path));
                return !fixture.RootElement.GetProperty("expected").GetProperty("isValid").GetBoolean();
            })
            .ToArray();

        Assert.NotEmpty(invalidFixturePaths);
        foreach (var fixturePath in invalidFixturePaths)
        {
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            using var exported = JsonDocument.Parse(ExecuteFixture(fixture.RootElement).ExportJson());

            AssertExpectedDiagnostics(fixture.RootElement, exported.RootElement);
        }
    }

    private static void AssertExpectedDiagnostics(JsonElement fixture, JsonElement exported)
    {
        var expected = fixture.GetProperty("expected").GetProperty("diagnostics").EnumerateArray()
            .Select(static code => code.GetString())
            .ToArray();
        var actual = exported.GetProperty("diagnostics").EnumerateArray()
            .Select(static diagnostic => diagnostic.GetProperty("code").GetString())
            .ToArray();

        Assert.Equal(expected, actual);
    }

    private static void AssertExpectedSummaries(JsonElement fixture, JsonElement exported)
    {
        var expected = fixture.GetProperty("expected").GetProperty("summaries").EnumerateArray().ToArray();
        var actual = exported.GetProperty("comparatorSummaries").EnumerateArray().ToArray();

        Assert.Equal(expected.Length, actual.Length);
        for (var i = 0; i < expected.Length; i++)
        {
            Assert.Equal(expected[i].GetProperty("comparatorName").GetString(), actual[i].GetProperty("comparatorName").GetString());
            Assert.Equal(expected[i].GetProperty("rowCount").GetInt32(), actual[i].GetProperty("rowCount").GetInt32());
        }
    }

    private static void AssertExpectedRows(JsonElement fixture, JsonElement exported)
    {
        var expectedRows = fixture.GetProperty("expected").GetProperty("rows");
        var actualRows = exported.GetProperty("rows");

        AssertRangeRows(expectedRows, actualRows, "overlap");
        AssertRangeRows(expectedRows, actualRows, "residual");
    }

    private static void AssertRangeRows(JsonElement expectedRows, JsonElement actualRows, string rowType)
    {
        if (!expectedRows.TryGetProperty(rowType, out var expected))
        {
            return;
        }

        var actual = actualRows.GetProperty(rowType).EnumerateArray().ToArray();
        var expectedArray = expected.EnumerateArray().ToArray();
        Assert.Equal(expectedArray.Length, actual.Length);

        for (var i = 0; i < expectedArray.Length; i++)
        {
            var expectedRow = expectedArray[i];
            var actualRow = actual[i];
            Assert.Equal(expectedRow.GetProperty("start").GetInt64(), actualRow.GetProperty("range").GetProperty("start").GetProperty("position").GetInt64());
            Assert.Equal(expectedRow.GetProperty("end").GetInt64(), actualRow.GetProperty("range").GetProperty("end").GetProperty("position").GetInt64());

            if (expectedRow.TryGetProperty("targetRecordCount", out var targetCount))
            {
                Assert.Equal(targetCount.GetInt32(), actualRow.GetProperty("targetRecordIds").GetArrayLength());
            }

            if (expectedRow.TryGetProperty("againstRecordCount", out var againstCount))
            {
                Assert.Equal(againstCount.GetInt32(), actualRow.GetProperty("againstRecordIds").GetArrayLength());
            }
        }
    }

    private static ComparisonResult ExecuteFixture(JsonElement fixture)
    {
        var history = CreateHistory(fixture.GetProperty("windows"));
        var plan = CreatePlan(fixture.GetProperty("plan"));
        return InvokeRuntime(Prepare(history, plan));
    }

    private static WindowHistory CreateHistory(JsonElement windows)
    {
        var constructor = typeof(WindowHistory).GetConstructor(
            BindingFlags.Instance | BindingFlags.NonPublic,
            binder: null,
            [typeof(bool)],
            modifiers: null)!;
        var history = (WindowHistory)constructor.Invoke([true]);
        var field = typeof(WindowHistory).GetField(
            "closedWindows",
            BindingFlags.Instance | BindingFlags.NonPublic)!;
        var closed = (List<ClosedWindow>)field.GetValue(history)!;

        foreach (var window in windows.EnumerateArray())
        {
            closed.Add(new ClosedWindow(
                window.GetProperty("windowName").GetString()!,
                window.GetProperty("key").GetString()!,
                window.GetProperty("startPosition").GetInt64(),
                window.GetProperty("endPosition").GetInt64(),
                Source: window.GetProperty("source").GetString(),
                Segments: ReadSegments(window),
                Tags: ReadTags(window)));
        }

        return history;
    }

    private static ComparisonPlan CreatePlan(JsonElement plan)
    {
        var against = ReadAgainstSelectors(plan);
        var scope = plan.GetProperty("scopeWindow").ValueKind == JsonValueKind.Null
            ? ComparisonScope.All()
            : ComparisonScope.Window(plan.GetProperty("scopeWindow").GetString()!);
        scope = ApplyScopeFilters(plan, scope);

        return new ComparisonPlan(
            plan.GetProperty("name").GetString()!,
            ComparisonSelector.ForSource(plan.GetProperty("targetSource").GetString()!),
            against,
            scope,
            ComparisonNormalizationPolicy.Default,
            plan.GetProperty("comparators").EnumerateArray().Select(static comparator => comparator.GetString()!),
            plan.GetProperty("strict").GetBoolean());
    }

    private static ComparisonSelector[] ReadAgainstSelectors(JsonElement plan)
    {
        if (plan.TryGetProperty("againstCohort", out var cohort)
            && cohort.ValueKind != JsonValueKind.Null)
        {
            var sources = cohort.GetProperty("sources").EnumerateArray()
                .Select(static source => source.GetString()!)
                .Cast<object>()
                .ToArray();

            return
            [
                ComparisonSelector
                    .ForCohortSources(sources, ReadCohortActivity(cohort))
                    .WithName(cohort.GetProperty("name").GetString()!)
            ];
        }

        return plan.GetProperty("againstSources").EnumerateArray()
            .Select(static source => ComparisonSelector.ForSource(source.GetString()!))
            .ToArray();
    }

    private static ComparisonScope ApplyScopeFilters(JsonElement plan, ComparisonScope scope)
    {
        if (plan.TryGetProperty("scopeSegments", out var segments))
        {
            foreach (var segment in segments.EnumerateArray())
            {
                scope = scope.Segment(
                    segment.GetProperty("name").GetString()!,
                    ReadPrimitive(segment.GetProperty("value")));
            }
        }

        if (plan.TryGetProperty("scopeTags", out var tags))
        {
            foreach (var tag in tags.EnumerateArray())
            {
                scope = scope.Tag(
                    tag.GetProperty("name").GetString()!,
                    ReadPrimitive(tag.GetProperty("value")));
            }
        }

        return scope;
    }

    private static CohortActivity ReadCohortActivity(JsonElement cohort)
    {
        var activity = cohort.TryGetProperty("activity", out var activityElement)
            ? activityElement.GetString()
            : "any";
        var count = cohort.TryGetProperty("count", out var countElement)
            && countElement.ValueKind != JsonValueKind.Null
                ? countElement.GetInt32()
                : 0;

        return activity switch
        {
            "any" => CohortActivity.Any(),
            "all" => CohortActivity.All(),
            "none" => CohortActivity.None(),
            "at-least" => CohortActivity.AtLeast(count),
            "at-most" => CohortActivity.AtMost(count),
            "exactly" => CohortActivity.Exactly(count),
            _ => throw new ArgumentException("Unsupported cohort activity: " + activity)
        };
    }

    private static IReadOnlyList<WindowSegment> ReadSegments(JsonElement window)
    {
        if (!window.TryGetProperty("segments", out var segments))
        {
            return [];
        }

        var values = new List<WindowSegment>();
        foreach (var segment in segments.EnumerateArray())
        {
            values.Add(new WindowSegment(
                segment.GetProperty("name").GetString()!,
                ReadPrimitive(segment.GetProperty("value")),
                segment.TryGetProperty("parentName", out var parentName)
                    && parentName.ValueKind != JsonValueKind.Null
                        ? parentName.GetString()
                        : null));
        }

        return values.ToArray();
    }

    private static IReadOnlyList<WindowTag> ReadTags(JsonElement window)
    {
        if (!window.TryGetProperty("tags", out var tags))
        {
            return [];
        }

        var values = new List<WindowTag>();
        foreach (var tag in tags.EnumerateArray())
        {
            values.Add(new WindowTag(
                tag.GetProperty("name").GetString()!,
                ReadPrimitive(tag.GetProperty("value"))));
        }

        return values.ToArray();
    }

    private static object? ReadPrimitive(JsonElement value)
    {
        return value.ValueKind switch
        {
            JsonValueKind.String => value.GetString(),
            JsonValueKind.Number => value.TryGetInt64(out var longValue) ? longValue : value.GetDouble(),
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            JsonValueKind.Null => null,
            _ => throw new ArgumentException("Fixture values must be string, number, boolean, or null.")
        };
    }

    private static PreparedComparison Prepare(WindowHistory history, ComparisonPlan plan)
    {
        var method = typeof(WindowComparisonBuilder)
            .Assembly
            .GetType("Spanfold.Internal.Comparison.ComparisonPreparer")!
            .GetMethod("Prepare", BindingFlags.Static | BindingFlags.NonPublic)!;

        return (PreparedComparison)method.Invoke(null, [history, plan])!;
    }

    private static ComparisonResult InvokeRuntime(PreparedComparison prepared)
    {
        var method = typeof(WindowComparisonBuilder)
            .Assembly
            .GetType("Spanfold.Internal.Comparison.ComparisonRuntime")!
            .GetMethod("Run", BindingFlags.Static | BindingFlags.NonPublic)!;

        return (ComparisonResult)method.Invoke(null, [prepared])!;
    }

    private static string[] FixturePaths([CallerFilePath] string callerFilePath = "")
    {
        return Directory.GetFiles(
                Path.Combine(Path.GetDirectoryName(callerFilePath)!, "Fixtures"),
                "*.json")
            .OrderBy(static path => path, StringComparer.Ordinal)
            .ToArray();
    }
}
