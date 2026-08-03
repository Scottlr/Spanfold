using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;

using Spanfold.Testing;

namespace Spanfold.Cli;

internal static class SpanfoldCli
{
    private static readonly JsonSerializerOptions JsonOutputOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true
    };

    static SpanfoldCli()
    {
        JsonOutputOptions.Converters.Add(new JsonStringEnumConverter());
    }

    public static int Run(string[] args, TextWriter stdout, TextWriter stderr)
    {
        ArgumentNullException.ThrowIfNull(args);
        ArgumentNullException.ThrowIfNull(stdout);
        ArgumentNullException.ThrowIfNull(stderr);

        try
        {
            if (args.Length < 2)
            {
                WriteError(stderr, "Usage: spanfold <validate-plan|compare|explain|audit|check|suite> <fixture.json> [options], spanfold verify-bundle <directory>, or spanfold diff <baseline> <current>.");
                return 2;
            }

            var command = args[0];
            if (!IsKnownCommand(command))
            {
                WriteError(stderr, "Unknown command: " + command);
                return 2;
            }

            if (string.Equals(command, "verify-bundle", StringComparison.Ordinal))
            {
                var verification = AuditBundleReader.Open(args[1]).Verify();
                stdout.Write(JsonSerializer.Serialize(verification, JsonOutputOptions));
                return verification.IsValid ? 0 : 1;
            }

            if (string.Equals(command, "diff", StringComparison.Ordinal))
            {
                if (args.Length != 3)
                {
                    throw new ArgumentException("The diff command requires <baseline> <current>.");
                }

                var revision = ComparisonArtifactRevision.Between(
                    ReadComparisonArtifact(args[1]),
                    ReadComparisonArtifact(args[2]));
                stdout.Write(JsonSerializer.Serialize(revision, JsonOutputOptions));
                return revision.IsEmpty ? 0 : 1;
            }

            ValidateOptions(args, command);

            if (string.Equals(command, "audit-windows", StringComparison.Ordinal))
            {
                var windowResult = ExecuteWindowJsonLines(args);
                var bundle = AuditBundleWriter.Write(ReadRequiredOption(args, "--out"), windowResult);
                stdout.Write(JsonSerializer.Serialize(bundle.Manifest, JsonOutputOptions));
                return windowResult.IsValid ? 0 : 1;
            }

            var fixturePath = args[1];
            var format = ReadFormat(args);
            using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
            var result = ContractFixtureRunner.Run(fixture.RootElement);

            if (string.Equals(command, "audit", StringComparison.Ordinal))
            {
                var bundle = AuditBundleWriter.Write(ReadRequiredOption(args, "--out"), result);
                stdout.Write(JsonSerializer.Serialize(bundle.Manifest, JsonOutputOptions));
                return result.IsValid ? 0 : 1;
            }

            if (string.Equals(command, "check", StringComparison.Ordinal))
            {
                var assessment = result.Assess(AssessmentDocument.ReadSpecification(ReadRequiredOption(args, "--spec")));
                stdout.Write(JsonSerializer.Serialize(assessment, JsonOutputOptions));
                return assessment.Passed ? 0 : 1;
            }

            if (string.Equals(command, "suite", StringComparison.Ordinal))
            {
                var suite = AssessmentDocument.ReadSuite(ReadRequiredOption(args, "--suite")).Evaluate(result);
                stdout.Write(JsonSerializer.Serialize(suite, JsonOutputOptions));
                return suite.Passed ? 0 : 1;
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
                or KeyNotFoundException
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
            || string.Equals(command, "audit-windows", StringComparison.Ordinal)
            || string.Equals(command, "check", StringComparison.Ordinal)
            || string.Equals(command, "suite", StringComparison.Ordinal)
            || string.Equals(command, "verify-bundle", StringComparison.Ordinal)
            || string.Equals(command, "diff", StringComparison.Ordinal);
    }

    private static void ValidateOptions(string[] args, string command)
    {
        var valueOptions = string.Equals(command, "audit-windows", StringComparison.Ordinal)
            ? new HashSet<string>(StringComparer.Ordinal)
            {
                "--target", "--against", "--out", "--window", "--comparators",
                "--name", "--live-horizon-position"
            }
            : new HashSet<string>(StringComparer.Ordinal) { "--format", "--out", "--spec", "--suite" };

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
        var builder = new WindowHistoryImportBuilder();
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

    private static ComparisonArtifact ReadComparisonArtifact(string path)
    {
        var fullPath = Path.GetFullPath(path);
        if (Directory.Exists(fullPath))
        {
            var bundle = AuditBundleReader.Open(fullPath);
            var verification = bundle.Verify();
            if (!verification.IsValid)
            {
                throw new InvalidDataException("The comparison bundle failed integrity verification.");
            }

            fullPath = Path.Combine(fullPath, "result.json");
        }

        return ComparisonArtifact.Read(fullPath);
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

    private static void WriteError(TextWriter writer, string message)
    {
        writer.Write("{\"error\":");
        writer.Write(JsonSerializer.Serialize(message));
        writer.Write('}');
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

internal static class WindowComparisonBuilderCliExtensions
{
    internal static WindowComparisonBuilder StrictIf(this WindowComparisonBuilder builder, bool isStrict)
    {
        return isStrict ? builder.Strict() : builder;
    }
}

internal sealed class WindowHistoryImportBuilder
{
    private readonly List<ClosedWindow> closed = [];
    private readonly List<OpenWindow> open = [];

    internal void AddClosedWindow(
        string windowName,
        object key,
        long startPosition,
        long endPosition,
        object? source,
        object? partition,
        IReadOnlyList<WindowSegment> segments,
        IReadOnlyList<WindowTag> tags)
    {
        this.closed.Add(new ClosedWindow(
            windowName,
            key,
            startPosition,
            endPosition,
            source,
            partition,
            Segments: segments,
            Tags: tags));
    }

    internal void AddOpenWindow(
        string windowName,
        object key,
        long startPosition,
        object? source,
        object? partition,
        IReadOnlyList<WindowSegment> segments,
        IReadOnlyList<WindowTag> tags)
    {
        this.open.Add(new OpenWindow(
            windowName,
            key,
            startPosition,
            source,
            partition,
            Segments: segments,
            Tags: tags));
    }

    internal WindowHistory Build() => WindowHistory.FromRecords(this.closed, this.open);
}
