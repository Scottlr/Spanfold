using System.Globalization;
using System.Text.Json;

using Spanfold.Testing;

namespace Spanfold.Cli;

public static class SpanfoldCli
{
    public static int Run(string[] args, TextWriter stdout, TextWriter stderr)
    {
        ArgumentNullException.ThrowIfNull(args);
        ArgumentNullException.ThrowIfNull(stdout);
        ArgumentNullException.ThrowIfNull(stderr);

        try
        {
            if (args.Length < 2)
            {
                WriteError(stderr, "Usage: spanfold <validate-plan|compare|explain|audit> <fixture.json> [--format json|markdown|llm-context] [--out directory] or spanfold audit-windows <windows.jsonl> --target source --against source --out directory [--window name]");
                return 2;
            }

            var command = args[0];
            if (!IsKnownCommand(command))
            {
                WriteError(stderr, "Unknown command: " + command);
                return 2;
            }

            ValidateOptions(args, command);

            if (string.Equals(command, "audit-windows", StringComparison.Ordinal))
            {
                var windowResult = ExecuteWindowJsonLines(args);
                stdout.Write(WriteAuditBundle(windowResult, ReadRequiredOption(args, "--out")));
                return windowResult.IsValid ? 0 : 1;
            }

            var fixturePath = args[1];
            var format = ReadFormat(args);
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            ValidateFixture(fixture.RootElement);
            var result = ExecuteFixture(fixture.RootElement);

            if (string.Equals(command, "audit", StringComparison.Ordinal))
            {
                stdout.Write(WriteAuditBundle(result, ReadRequiredOption(args, "--out")));
                return result.IsValid ? 0 : 1;
            }

            if (string.Equals(command, "validate-plan", StringComparison.Ordinal))
            {
                WriteDiagnostics(stdout, result);
                return result.IsValid ? 0 : 1;
            }

            if (string.Equals(command, "compare", StringComparison.Ordinal))
            {
                stdout.Write(format switch
                {
                    "markdown" => result.ExportMarkdown(),
                    "llm-context" => result.ExportLlmContext(),
                    _ => result.ExportJson()
                });
                return result.IsValid ? 0 : 1;
            }

            if (string.Equals(command, "explain", StringComparison.Ordinal))
            {
                stdout.Write(result.ExportMarkdown());
                return result.IsValid ? 0 : 1;
            }

            return 2;
        }
        catch (Exception exception) when (
            exception is IOException
                or JsonException
                or ArgumentException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            WriteError(stderr, exception.Message);
            return 2;
        }
    }

    private static bool IsKnownCommand(string command)
    {
        return string.Equals(command, "validate-plan", StringComparison.Ordinal)
            || string.Equals(command, "compare", StringComparison.Ordinal)
            || string.Equals(command, "explain", StringComparison.Ordinal)
            || string.Equals(command, "audit", StringComparison.Ordinal)
            || string.Equals(command, "audit-windows", StringComparison.Ordinal);
    }

    private static void ValidateOptions(string[] args, string command)
    {
        var valueOptions = string.Equals(command, "audit-windows", StringComparison.Ordinal)
            ? new HashSet<string>(StringComparer.Ordinal)
            {
                "--target", "--against", "--out", "--window", "--comparators",
                "--name", "--live-horizon-position"
            }
            : new HashSet<string>(StringComparer.Ordinal) { "--format", "--out" };

        var flags = new HashSet<string>(StringComparer.Ordinal) { "--strict" };
        for (var index = 2; index < args.Length; index++)
        {
            var option = args[index];
            if (!option.StartsWith("--", StringComparison.Ordinal))
            {
                throw new ArgumentException("Unexpected positional argument: " + option);
            }

            if (!valueOptions.Contains(option) && !flags.Contains(option))
            {
                throw new ArgumentException("Unknown option: " + option);
            }

            if (valueOptions.Contains(option))
            {
                if (index + 1 >= args.Length || args[index + 1].StartsWith("--", StringComparison.Ordinal))
                {
                    throw new ArgumentException("Option " + option + " requires a value.");
                }

                index++;
            }
        }
    }

    private static string ReadFormat(string[] args)
    {
        for (var i = 2; i < args.Length - 1; i++)
        {
            if (string.Equals(args[i], "--format", StringComparison.Ordinal))
            {
                var format = args[i + 1];
                if (string.Equals(format, "json", StringComparison.Ordinal)
                    || string.Equals(format, "markdown", StringComparison.Ordinal)
                    || string.Equals(format, "llm-context", StringComparison.Ordinal))
                {
                    return format;
                }

                throw new ArgumentException("Unsupported format: " + format);
            }
        }

        return "json";
    }

    private static string? ReadOptionalOption(string[] args, string optionName)
    {
        for (var i = 2; i < args.Length - 1; i++)
        {
            if (string.Equals(args[i], optionName, StringComparison.Ordinal))
            {
                return string.IsNullOrWhiteSpace(args[i + 1]) ? null : args[i + 1];
            }
        }

        return null;
    }

    private static string ReadRequiredOption(string[] args, string optionName)
    {
        for (var i = 2; i < args.Length - 1; i++)
        {
            if (string.Equals(args[i], optionName, StringComparison.Ordinal))
            {
                ArgumentException.ThrowIfNullOrWhiteSpace(args[i + 1]);
                return args[i + 1];
            }
        }

        throw new ArgumentException("The command requires " + optionName + " <value>.");
    }

    private static IReadOnlyList<string> ReadOptionValues(string[] args, string optionName)
    {
        var values = new List<string>();
        for (var i = 2; i < args.Length - 1; i++)
        {
            if (!string.Equals(args[i], optionName, StringComparison.Ordinal))
            {
                continue;
            }

            values.AddRange(args[i + 1]
                .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Where(static value => value.Length > 0));
        }

        return values;
    }

    private static ComparisonResult ExecuteFixture(JsonElement fixture)
    {
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
            .Normalize(_ => BuildNormalization(plan.Normalization))
            .StrictIf(plan.IsStrict);

        return liveHorizon.HasValue
            ? builder.RunLive(liveHorizon.Value)
            : builder.Run();
    }

    private static ComparisonResult ExecuteWindowJsonLines(string[] args)
    {
        var path = args[1];
        var target = ReadRequiredOption(args, "--target");
        var againstSources = ReadOptionValues(args, "--against");
        if (againstSources.Count == 0)
        {
            throw new ArgumentException("The audit-windows command requires --against <source>.");
        }

        var windowName = ReadOptionalOption(args, "--window");
        var comparators = ReadOptionValues(args, "--comparators");
        if (comparators.Count == 0)
        {
            comparators = ["overlap", "residual", "coverage"];
        }

        var history = CreateHistoryFromWindowJsonLines(path, windowName);
        var comparisonName = ReadOptionalOption(args, "--name") ?? "Spanfold Window Audit";
        var builder = history.Compare(comparisonName)
            .Target(target, selector => selector.Source(target));

        foreach (var source in againstSources)
        {
            builder.Against(source, selector => selector.Source(source));
        }

        var scope = string.IsNullOrWhiteSpace(windowName)
            ? ComparisonScope.All()
            : ComparisonScope.Window(windowName);

        builder = builder
            .Within(_ => scope)
            .Using(_ => BuildComparators(comparators))
            .StrictIf(HasFlag(args, "--strict"));

        var horizon = ReadOptionalOption(args, "--live-horizon-position");
        return horizon is null
            ? builder.Run()
            : builder.RunLive(TemporalPoint.ForPosition(long.Parse(horizon, CultureInfo.InvariantCulture)));
    }

    private static WindowHistory CreateHistoryFromWindowJsonLines(string path, string? defaultWindowName)
    {
        var builder = new WindowHistoryFixtureBuilder();
        var lineNumber = 0;
        foreach (var line in File.ReadLines(path))
        {
            lineNumber++;
            if (string.IsNullOrWhiteSpace(line))
            {
                continue;
            }

            using var document = JsonDocument.Parse(line);
            var window = document.RootElement;
            RequireKind(window, "$", JsonValueKind.Object);
            var windowName = window.TryGetProperty("windowName", out var rowWindowName)
                && rowWindowName.ValueKind != JsonValueKind.Null
                    ? rowWindowName.GetString()
                    : defaultWindowName;
            if (string.IsNullOrWhiteSpace(windowName))
            {
                throw new ArgumentException("windows.jsonl line " + lineNumber.ToString(CultureInfo.InvariantCulture) + " must include windowName or use --window.");
            }

            var key = RequireProperty(window, "key", "$", JsonValueKind.String).GetString()!;
            var source = RequireProperty(window, "source", "$", JsonValueKind.String).GetString();
            var startPosition = RequireProperty(window, "startPosition", "$", JsonValueKind.Number).GetInt64();
            var partition = window.TryGetProperty("partition", out var partitionProperty)
                && partitionProperty.ValueKind != JsonValueKind.Null
                    ? partitionProperty.GetString()
                    : null;
            var segments = ReadSegments(window);
            var tags = ReadTags(window);
            if (!window.TryGetProperty("endPosition", out var endPosition)
                || endPosition.ValueKind == JsonValueKind.Null)
            {
                builder.AddOpenWindow(windowName, key, startPosition, source, partition, segments, tags);
                continue;
            }

            RequireKind(endPosition, "$.endPosition", JsonValueKind.Number);
            builder.AddClosedWindow(
                windowName,
                key,
                startPosition,
                endPosition.GetInt64(),
                source,
                partition,
                segments,
                tags);
        }

        return builder.Build();
    }

    private static bool HasFlag(string[] args, string flag)
    {
        for (var i = 2; i < args.Length; i++)
        {
            if (string.Equals(args[i], flag, StringComparison.Ordinal))
            {
                return true;
            }
        }

        return false;
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
            ComparisonOutputOptions.Default,
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

    private static void WriteDiagnostics(TextWriter writer, ComparisonResult result)
    {
        writer.Write("{\"isValid\":");
        writer.Write(result.IsValid ? "true" : "false");
        writer.Write(",\"diagnostics\":[");
        for (var i = 0; i < result.Diagnostics.Count; i++)
        {
            if (i > 0)
            {
                writer.Write(',');
            }

            writer.Write('"');
            writer.Write(result.Diagnostics[i].Code.ToString());
            writer.Write('"');
        }

        writer.Write("]}");
    }

    private static string WriteAuditBundle(ComparisonResult result, string outputDirectory)
    {
        var fullDirectory = Path.GetFullPath(outputDirectory);

        const string jsonFileName = "comparison.json";
        const string markdownFileName = "comparison.md";
        const string debugHtmlFileName = "comparison.html";
        const string llmContextFileName = "comparison.llm.json";
        const string manifestFileName = "manifest.json";

        var manifest = ExportAuditManifest(
            result,
            new AuditArtifacts(
                jsonFileName,
                markdownFileName,
                debugHtmlFileName,
                llmContextFileName,
                manifestFileName));
        var parentDirectory = Directory.GetParent(fullDirectory)?.FullName
            ?? throw new InvalidOperationException("Audit bundle output must have a parent directory.");
        Directory.CreateDirectory(parentDirectory);
        var temporaryDirectory = Path.Combine(parentDirectory, "." + Path.GetFileName(fullDirectory) + ".tmp-" + Guid.NewGuid().ToString("N"));
        var backupDirectory = Path.Combine(parentDirectory, "." + Path.GetFileName(fullDirectory) + ".old-" + Guid.NewGuid().ToString("N"));
        try
        {
            Directory.CreateDirectory(temporaryDirectory);
            File.WriteAllText(Path.Combine(temporaryDirectory, jsonFileName), result.ExportJson());
            File.WriteAllText(Path.Combine(temporaryDirectory, markdownFileName), result.ExportMarkdown());
            File.WriteAllText(Path.Combine(temporaryDirectory, debugHtmlFileName), result.ExportDebugHtml());
            File.WriteAllText(Path.Combine(temporaryDirectory, llmContextFileName), result.ExportLlmContext());
            File.WriteAllText(Path.Combine(temporaryDirectory, manifestFileName), manifest);

            if (Directory.Exists(fullDirectory))
            {
                Directory.Move(fullDirectory, backupDirectory);
            }

            Directory.Move(temporaryDirectory, fullDirectory);
            if (Directory.Exists(backupDirectory))
            {
                Directory.Delete(backupDirectory, recursive: true);
            }
        }
        catch
        {
            if (!Directory.Exists(fullDirectory) && Directory.Exists(backupDirectory))
            {
                Directory.Move(backupDirectory, fullDirectory);
            }

            throw;
        }
        finally
        {
            if (Directory.Exists(temporaryDirectory))
            {
                Directory.Delete(temporaryDirectory, recursive: true);
            }

            if (Directory.Exists(backupDirectory))
            {
                Directory.Delete(backupDirectory, recursive: true);
            }
        }

        return manifest;
    }

    private static string ExportAuditManifest(ComparisonResult result, AuditArtifacts artifacts)
    {
        var manifest = new AuditManifest(
            "spanfold.audit.bundle",
            0,
            "audit-bundle",
            result.Plan.Name,
            result.IsValid,
            result.Diagnostics.Count,
            result.RowFinalities.Count(static row => row.Finality == ComparisonFinality.Provisional),
            new AuditRowCounts(
                result.OverlapRows.Count,
                result.ResidualRows.Count,
                result.MissingRows.Count,
                result.CoverageRows.Count,
                result.GapRows.Count,
                result.SymmetricDifferenceRows.Count,
                result.ContainmentRows.Count,
                result.LeadLagRows.Count,
                result.AsOfRows.Count),
            artifacts);

        return JsonSerializer.Serialize(
            manifest,
            new JsonSerializerOptions
            {
                PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                WriteIndented = true
            });
    }

    private static void WriteError(TextWriter writer, string message)
    {
        writer.Write("{\"error\":");
        writer.Write(JsonSerializer.Serialize(message));
        writer.Write('}');
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
            var itemValue = RequireProperty(
                value,
                "value",
                itemPath,
                JsonValueKind.String,
                JsonValueKind.Number,
                JsonValueKind.True,
                JsonValueKind.False,
                JsonValueKind.Null);

            _ = itemValue;

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

internal sealed record AuditManifest(
    string Schema,
    int SchemaVersion,
    string Artifact,
    string PlanName,
    bool IsValid,
    int DiagnosticCount,
    int ProvisionalRowCount,
    AuditRowCounts RowCounts,
    AuditArtifacts Artifacts);

internal sealed record AuditRowCounts(
    int Overlap,
    int Residual,
    int Missing,
    int Coverage,
    int Gap,
    int SymmetricDifference,
    int Containment,
    int LeadLag,
    int AsOf);

internal sealed record AuditArtifacts(
    string Json,
    string Markdown,
    string DebugHtml,
    string LlmContext,
    string Manifest);

internal static class WindowComparisonBuilderCliExtensions
{
    internal static WindowComparisonBuilder StrictIf(this WindowComparisonBuilder builder, bool isStrict)
    {
        return isStrict ? builder.Strict() : builder;
    }
}
