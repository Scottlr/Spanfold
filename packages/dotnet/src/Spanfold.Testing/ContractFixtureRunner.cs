using System.Globalization;
using System.Text.Json;

namespace Spanfold.Testing;

/// <summary>
/// Validates and executes Spanfold contract fixtures using schema version 1.
/// </summary>
public static class ContractFixtureRunner
{
    /// <summary>
    /// Validates, constructs, and executes a contract fixture.
    /// </summary>
    /// <param name="fixture">The contract fixture JSON root.</param>
    /// <returns>The comparison result produced by the fixture plan.</returns>
    /// <exception cref="ArgumentException">The fixture does not conform to the supported schema.</exception>
    public static ComparisonResult Run(JsonElement fixture)
    {
        ValidateFixture(fixture);

        var history = CreateHistory(fixture.GetProperty("windows"));
        var plan = CreatePlan(fixture.GetProperty("plan"));
        var liveHorizon = ReadLiveHorizon(fixture.GetProperty("plan"));
        var builder = history.Compare(plan.Name)
            .Target(plan.Target!.Value.Name, _ => plan.Target.Value);

        for (var i = 0; i < plan.Against.Count; i++)
        {
            var selector = plan.Against[i];
            builder.Against(selector.Name, _ => selector);
        }

        builder = builder
            .Within(_ => plan.Scope!)
            .Using(_ => BuildComparators(plan.Comparators))
            .Normalize(_ => BuildNormalization(plan.Normalization));

        if (plan.IsStrict)
        {
            builder = builder.Strict();
        }

        return liveHorizon.HasValue
            ? builder.RunLive(liveHorizon.Value)
            : builder.Run();
    }

    private static WindowHistory CreateHistory(JsonElement windows)
    {
        var builder = new WindowHistoryFixtureBuilder();
        foreach (var window in windows.EnumerateArray())
        {
            var windowName = window.GetProperty("windowName").GetString()!;
            var key = window.GetProperty("key").GetString()!;
            var startPosition = window.GetProperty("startPosition").GetInt64();
            var source = window.GetProperty("source").GetString();
            var partition = window.TryGetProperty("partition", out var partitionProperty)
                && partitionProperty.ValueKind != JsonValueKind.Null
                    ? partitionProperty.GetString()
                    : null;
            var segments = ReadSegments(window);
            var tags = ReadTags(window);

            if (window.GetProperty("endPosition").ValueKind == JsonValueKind.Null)
            {
                builder.AddOpenWindow(windowName, key, startPosition, source, partition, segments, tags);
                continue;
            }

            builder.AddClosedWindow(
                windowName,
                key,
                startPosition,
                window.GetProperty("endPosition").GetInt64(),
                source,
                partition,
                segments,
                tags);
        }

        return builder.Build();
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
            var activity = ReadCohortActivity(cohort);
            var sources = cohort.GetProperty("sources").EnumerateArray()
                .Select(static source => source.GetString()!)
                .Cast<object>()
                .ToArray();
            return
            [
                ComparisonSelector
                    .ForCohortSources(sources, activity)
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

    private static TemporalPoint? ReadLiveHorizon(JsonElement plan)
    {
        return plan.TryGetProperty("liveHorizonPosition", out var horizon)
            && horizon.ValueKind != JsonValueKind.Null
                ? TemporalPoint.ForPosition(horizon.GetInt64())
                : null;
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

    private static ComparisonComparatorBuilder BuildComparators(IReadOnlyList<string> comparators)
    {
        var builder = new ComparisonComparatorBuilder();
        for (var i = 0; i < comparators.Count; i++)
        {
            builder.Declaration(comparators[i]);
        }

        return builder;
    }

    private static ComparisonNormalizationBuilder BuildNormalization(ComparisonNormalizationPolicy policy)
    {
        var builder = new ComparisonNormalizationBuilder();
        if (policy.TimeAxis == TemporalAxis.Timestamp)
        {
            builder.OnEventTime();
        }

        if (policy.OpenWindowPolicy == ComparisonOpenWindowPolicy.ClipToHorizon
            && policy.OpenWindowHorizon.HasValue)
        {
            builder.ClipOpenWindowsTo(policy.OpenWindowHorizon.Value);
        }

        return builder;
    }

    private static void ValidateFixture(JsonElement fixture)
    {
        RequireKind(fixture, "$", JsonValueKind.Object);
        var schema = RequireProperty(fixture, "schema", "$", JsonValueKind.String);
        if (!string.Equals(schema.GetString(), "spanfold.contract-fixture", StringComparison.Ordinal))
        {
            throw new ArgumentException("Unsupported fixture schema. Expected spanfold.contract-fixture.");
        }

        var schemaVersion = RequireProperty(fixture, "schemaVersion", "$", JsonValueKind.Number);
        if (!schemaVersion.TryGetInt32(out var version) || version != 1)
        {
            throw new ArgumentException("Unsupported fixture schemaVersion. Expected version 1.");
        }

        var windows = RequireProperty(fixture, "windows", "$", JsonValueKind.Array);
        var plan = RequireProperty(fixture, "plan", "$", JsonValueKind.Object);

        ValidateWindows(windows);
        ValidatePlan(plan);
    }

    private static void ValidateWindows(JsonElement windows)
    {
        var index = 0;
        foreach (var window in windows.EnumerateArray())
        {
            var path = "$.windows[" + index.ToString(CultureInfo.InvariantCulture) + "]";
            RequireKind(window, path, JsonValueKind.Object);
            RequireProperty(window, "windowName", path, JsonValueKind.String);
            RequireProperty(window, "key", path, JsonValueKind.String);
            RequireProperty(window, "source", path, JsonValueKind.String);
            var start = RequireProperty(window, "startPosition", path, JsonValueKind.Number).GetInt64();
            var end = RequireProperty(window, "endPosition", path, JsonValueKind.Number, JsonValueKind.Null);

            if (end.ValueKind == JsonValueKind.Number && end.GetInt64() < start)
            {
                throw new ArgumentException(path + ".endPosition must be greater than or equal to startPosition.");
            }

            if (window.TryGetProperty("partition", out var partition))
            {
                RequireKind(partition, path + ".partition", JsonValueKind.String, JsonValueKind.Null);
            }

            if (window.TryGetProperty("segments", out var segments))
            {
                ValidateNamedValues(segments, path + ".segments", allowParentName: true);
            }

            if (window.TryGetProperty("tags", out var tags))
            {
                ValidateNamedValues(tags, path + ".tags", allowParentName: false);
            }

            index++;
        }
    }

    private static void ValidatePlan(JsonElement plan)
    {
        RequireProperty(plan, "name", "$.plan", JsonValueKind.String);
        RequireProperty(plan, "targetSource", "$.plan", JsonValueKind.String);
        var hasAgainstSources = plan.TryGetProperty("againstSources", out var against);
        var hasAgainstCohort = plan.TryGetProperty("againstCohort", out var cohort)
            && cohort.ValueKind != JsonValueKind.Null;
        if (!hasAgainstSources && !hasAgainstCohort)
        {
            throw new ArgumentException("$.plan must contain againstSources or againstCohort.");
        }

        if (hasAgainstSources)
        {
            RequireKind(against, "$.plan.againstSources", JsonValueKind.Array);
            if (against.GetArrayLength() == 0 && !hasAgainstCohort)
            {
                throw new ArgumentException("$.plan.againstSources must contain at least one source.");
            }

            var againstIndex = 0;
            foreach (var source in against.EnumerateArray())
            {
                RequireKind(
                    source,
                    "$.plan.againstSources[" + againstIndex.ToString(CultureInfo.InvariantCulture) + "]",
                    JsonValueKind.String);
                againstIndex++;
            }
        }

        if (hasAgainstCohort)
        {
            ValidateCohort(cohort);
        }

        RequireProperty(plan, "scopeWindow", "$.plan", JsonValueKind.String, JsonValueKind.Null);
        if (plan.TryGetProperty("scopeSegments", out var scopeSegments))
        {
            ValidateNamedValues(scopeSegments, "$.plan.scopeSegments", allowParentName: false);
        }

        if (plan.TryGetProperty("scopeTags", out var scopeTags))
        {
            ValidateNamedValues(scopeTags, "$.plan.scopeTags", allowParentName: false);
        }

        var comparators = RequireProperty(plan, "comparators", "$.plan", JsonValueKind.Array);
        if (comparators.GetArrayLength() == 0)
        {
            throw new ArgumentException("$.plan.comparators must contain at least one comparator.");
        }

        var comparatorIndex = 0;
        foreach (var comparator in comparators.EnumerateArray())
        {
            RequireKind(
                comparator,
                "$.plan.comparators[" + comparatorIndex.ToString(CultureInfo.InvariantCulture) + "]",
                JsonValueKind.String);
            comparatorIndex++;
        }

        RequireProperty(plan, "strict", "$.plan", JsonValueKind.True, JsonValueKind.False);
        if (plan.TryGetProperty("liveHorizonPosition", out var horizon))
        {
            RequireKind(horizon, "$.plan.liveHorizonPosition", JsonValueKind.Number, JsonValueKind.Null);
        }
    }

    private static void ValidateCohort(JsonElement cohort)
    {
        RequireKind(cohort, "$.plan.againstCohort", JsonValueKind.Object);
        RequireProperty(cohort, "name", "$.plan.againstCohort", JsonValueKind.String);
        var sources = RequireProperty(cohort, "sources", "$.plan.againstCohort", JsonValueKind.Array);
        if (sources.GetArrayLength() == 0)
        {
            throw new ArgumentException("$.plan.againstCohort.sources must contain at least one source.");
        }

        var index = 0;
        foreach (var source in sources.EnumerateArray())
        {
            RequireKind(
                source,
                "$.plan.againstCohort.sources[" + index.ToString(CultureInfo.InvariantCulture) + "]",
                JsonValueKind.String);
            index++;
        }

        if (cohort.TryGetProperty("activity", out var activity))
        {
            RequireKind(activity, "$.plan.againstCohort.activity", JsonValueKind.String);
        }

        if (cohort.TryGetProperty("count", out var count))
        {
            RequireKind(count, "$.plan.againstCohort.count", JsonValueKind.Number, JsonValueKind.Null);
        }
    }

    private static void ValidateNamedValues(JsonElement values, string path, bool allowParentName)
    {
        RequireKind(values, path, JsonValueKind.Array);

        var index = 0;
        foreach (var value in values.EnumerateArray())
        {
            var itemPath = path + "[" + index.ToString(CultureInfo.InvariantCulture) + "]";
            RequireKind(value, itemPath, JsonValueKind.Object);
            RequireProperty(value, "name", itemPath, JsonValueKind.String);
            RequireProperty(
                value,
                "value",
                itemPath,
                JsonValueKind.String,
                JsonValueKind.Number,
                JsonValueKind.True,
                JsonValueKind.False,
                JsonValueKind.Null);

            if (allowParentName && value.TryGetProperty("parentName", out var parentName))
            {
                RequireKind(parentName, itemPath + ".parentName", JsonValueKind.String, JsonValueKind.Null);
            }

            index++;
        }
    }

    private static JsonElement RequireProperty(
        JsonElement element,
        string propertyName,
        string path,
        params JsonValueKind[] expectedKinds)
    {
        if (!element.TryGetProperty(propertyName, out var property))
        {
            throw new ArgumentException(path + " is missing required property '" + propertyName + "'.");
        }

        RequireKind(property, path + "." + propertyName, expectedKinds);
        return property;
    }

    private static void RequireKind(JsonElement element, string path, params JsonValueKind[] expectedKinds)
    {
        for (var i = 0; i < expectedKinds.Length; i++)
        {
            if (element.ValueKind == expectedKinds[i])
            {
                return;
            }
        }

        throw new ArgumentException(path + " has unsupported JSON kind " + element.ValueKind + ".");
    }
}
