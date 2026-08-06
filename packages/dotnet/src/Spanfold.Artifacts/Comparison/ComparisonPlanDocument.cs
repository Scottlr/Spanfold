using System.Globalization;
using System.Text.Json;

using Spanfold.Comparison;

namespace Spanfold.Artifacts.Comparison;

/// <summary>Represents a versioned portable comparison plan that can be parsed, written, and compiled.</summary>
public sealed record ComparisonPlanDocument
{
    private const string Schema = "spanfold.comparison.plan";
    private const int SchemaVersion = 0;

    private ComparisonPlanDocument(
        string name,
        ComparisonPlanSelectorDocument? target,
        IReadOnlyList<ComparisonPlanSelectorDocument> against,
        ComparisonScope? scope,
        ComparisonNormalizationPolicy normalization,
        IReadOnlyList<string> comparators,
        bool isStrict)
    {
        Name = name;
        Target = target;
        Against = against;
        Scope = scope;
        Normalization = normalization;
        Comparators = comparators;
        IsStrict = isStrict;
    }

    /// <summary>Gets the plan name.</summary>
    public string Name { get; }

    /// <summary>Gets the target selector document, when present.</summary>
    public ComparisonPlanSelectorDocument? Target { get; }

    /// <summary>Gets the comparison selector documents in declaration order.</summary>
    public IReadOnlyList<ComparisonPlanSelectorDocument> Against { get; }

    /// <summary>Gets the comparison scope, when present.</summary>
    public ComparisonScope? Scope { get; }

    /// <summary>Gets the normalization policy.</summary>
    public ComparisonNormalizationPolicy Normalization { get; }

    /// <summary>Gets comparator declarations in declaration order.</summary>
    public IReadOnlyList<string> Comparators { get; }

    /// <summary>Gets whether strict validation is enabled.</summary>
    public bool IsStrict { get; }

    /// <summary>Creates a portable document from an existing serializable plan.</summary>
    /// <param name="plan">The plan to represent.</param>
    /// <returns>The portable document.</returns>
    public static ComparisonPlanDocument FromPlan(ComparisonPlan plan)
    {
        ArgumentNullException.ThrowIfNull(plan);
        if (!plan.IsSerializable)
        {
            _ = plan.ExportPortableJson();
        }

        var target = plan.Target.HasValue
            ? ComparisonPlanSelectorDocument.FromSelector(plan.Target.Value)
            : null;

        return new ComparisonPlanDocument(
            plan.Name,
            target,
            plan.Against.Select(ComparisonPlanSelectorDocument.FromSelector).ToArray(),
            plan.Scope,
            plan.Normalization,
            plan.Comparators.ToArray(),
            plan.IsStrict);
    }

    /// <summary>Parses and validates a portable comparison plan JSON document.</summary>
    /// <param name="json">The document JSON.</param>
    /// <returns>The parsed document.</returns>
    public static ComparisonPlanDocument Parse(string json)
    {
        ArgumentNullException.ThrowIfNull(json);

        using var parsed = JsonDocument.Parse(json);
        var root = parsed.RootElement;
        RequireKind(root, "$", JsonValueKind.Object);
        RequireExactString(root, "schema", "$", Schema);
        RequireExactInt32(root, "schemaVersion", "$", SchemaVersion);
        RequireExactString(root, "artifact", "$", "plan");

        var targetElement = RequireProperty(root, "target", "$", JsonValueKind.Object, JsonValueKind.Null);
        var target = targetElement.ValueKind == JsonValueKind.Null
            ? null
            : ReadSelector(targetElement, "$.target");

        var againstElement = RequireProperty(root, "against", "$", JsonValueKind.Array);
        var against = againstElement.EnumerateArray()
            .Select((selector, index) => ReadSelector(selector, "$.against[" + index.ToString(CultureInfo.InvariantCulture) + "]"))
            .ToArray();

        var scopeElement = RequireProperty(root, "scope", "$", JsonValueKind.Object, JsonValueKind.Null);
        var scope = scopeElement.ValueKind == JsonValueKind.Null ? null : ReadScope(scopeElement);
        var normalization = ReadNormalization(RequireProperty(root, "normalization", "$", JsonValueKind.Object));
        var comparators = ReadStrings(RequireProperty(root, "comparators", "$", JsonValueKind.Array), "$.comparators");
        var isStrict = RequireProperty(root, "isStrict", "$", JsonValueKind.True, JsonValueKind.False).GetBoolean();

        return new ComparisonPlanDocument(
            RequireString(root, "name", "$"),
            target,
            against,
            scope,
            normalization,
            comparators,
            isStrict);
    }

    /// <summary>Reads and parses a portable comparison plan JSON file.</summary>
    /// <param name="path">The document path.</param>
    /// <returns>The parsed document.</returns>
    public static ComparisonPlanDocument Read(string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        return Parse(File.ReadAllText(Path.GetFullPath(path)));
    }

    /// <summary>Writes deterministic portable JSON using the comparison-plan artifact contract.</summary>
    /// <returns>The portable JSON.</returns>
    public string WriteJson()
    {
        return Compile().ExportPortableJson();
    }

    /// <summary>Writes deterministic portable JSON to a file.</summary>
    /// <param name="path">The destination path.</param>
    public void Write(string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        File.WriteAllText(Path.GetFullPath(path), WriteJson());
    }

    /// <summary>Compiles the document into the existing executable comparison plan API.</summary>
    /// <returns>The compiled plan.</returns>
    public ComparisonPlan Compile()
    {
        return new ComparisonPlan(
            Name,
            Target?.Compile("$.target"),
            Against.Select((selector, index) => selector.Compile("$.against[" + index.ToString(CultureInfo.InvariantCulture) + "]")),
            Scope,
            Normalization,
            Comparators,
            IsStrict);
    }

    private static ComparisonPlanSelectorDocument ReadSelector(JsonElement root, string path)
    {
        RequireKind(root, path, JsonValueKind.Object);
        var name = RequireString(root, "name", path);
        var descriptor = ReadDescriptor(RequireProperty(root, "descriptor", path, JsonValueKind.Object), path + ".descriptor");
        return new ComparisonPlanSelectorDocument(name, descriptor);
    }

    private static ComparisonSelectorDescriptor ReadDescriptor(JsonElement root, string path)
    {
        var children = root.TryGetProperty("children", out var childrenElement)
            ? ReadDescriptors(childrenElement, path + ".children")
            : [];

        return new ComparisonSelectorDescriptor(
            RequireString(root, "kind", path),
            root.TryGetProperty("value", out var value) ? ReadPortableValue(value, path + ".value") : null,
            root.TryGetProperty("values", out var values) ? ReadPortableValues(values, path + ".values") : null,
            ReadNullableInt64(root, "startPosition", path),
            ReadNullableInt64(root, "endPosition", path),
            ReadNullableTimestamp(root, "startTime", path),
            ReadNullableTimestamp(root, "endTime", path),
            ReadNullableString(root, "clock", path),
            ReadNullableString(root, "activity", path),
            ReadNullableInt32(root, "count", path),
            children);
    }

    private static IReadOnlyList<ComparisonSelectorDescriptor> ReadDescriptors(JsonElement element, string path)
    {
        RequireKind(element, path, JsonValueKind.Array);
        return element.EnumerateArray()
            .Select((child, index) => ReadDescriptor(child, path + "[" + index.ToString(CultureInfo.InvariantCulture) + "]"))
            .ToArray();
    }

    private static ComparisonScope ReadScope(JsonElement root)
    {
        var windowName = ReadNullableString(root, "windowName", "$.scope");
        var timeAxis = ReadEnum<TemporalAxis>(RequireString(root, "timeAxis", "$.scope"), "$.scope.timeAxis");
        var segmentFilters = ReadFilters(root, "segmentFilters", "$.scope");
        var tagFilters = ReadFilters(root, "tagFilters", "$.scope");
        return new ComparisonScope(
            windowName,
            timeAxis,
            segmentFilters.Select(static filter => new WindowSegmentFilter(filter.Name, filter.Value)).ToArray(),
            tagFilters.Select(static filter => new WindowTagFilter(filter.Name, filter.Value)).ToArray());
    }

    private static ComparisonNormalizationPolicy ReadNormalization(JsonElement root)
    {
        return new ComparisonNormalizationPolicy(
            ReadEnum<TemporalAxis>(RequireString(root, "timeAxis", "$.normalization"), "$.normalization.timeAxis"),
            ReadEnum<ComparisonOpenWindowPolicy>(RequireString(root, "openWindowPolicy", "$.normalization"), "$.normalization.openWindowPolicy"),
            ReadPoint(RequireProperty(root, "openWindowHorizon", "$.normalization", JsonValueKind.Object, JsonValueKind.Null), "$.normalization.openWindowHorizon"),
            ReadEnum<ComparisonNullTimestampPolicy>(RequireString(root, "nullTimestampPolicy", "$.normalization"), "$.normalization.nullTimestampPolicy"),
            ReadPoint(RequireProperty(root, "knownAt", "$.normalization", JsonValueKind.Object, JsonValueKind.Null), "$.normalization.knownAt"));
    }

    private static TemporalPoint? ReadPoint(JsonElement root, string path)
    {
        if (root.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        var axis = ReadEnum<TemporalAxis>(RequireString(root, "axis", path), path + ".axis");
        return axis switch
        {
            TemporalAxis.ProcessingPosition => TemporalPoint.ForPosition(RequireInt64(root, "position", path)),
            TemporalAxis.Timestamp => TemporalPoint.ForTimestamp(
                RequireTimestamp(root, "timestamp", path),
                ReadNullableString(root, "clock", path)),
            _ => throw new InvalidDataException(path + " uses unsupported temporal axis '" + axis + "'.")
        };
    }

    private static IReadOnlyList<ComparisonPlanFilter> ReadFilters(JsonElement root, string propertyName, string path)
    {
        if (!root.TryGetProperty(propertyName, out var filters))
        {
            return [];
        }

        RequireKind(filters, path + "." + propertyName, JsonValueKind.Array);
        return filters.EnumerateArray()
            .Select((filter, index) => new ComparisonPlanFilter(
                RequireString(filter, "name", path + "." + propertyName + "[" + index.ToString(CultureInfo.InvariantCulture) + "]"),
                ReadPortableValue(RequireProperty(filter, "value", path, JsonValueKind.Object, JsonValueKind.Null), path + "." + propertyName + "[" + index.ToString(CultureInfo.InvariantCulture) + "].value")))
            .ToArray();
    }

    private static object? ReadPortableValue(JsonElement root, string path)
    {
        if (root.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        RequireKind(root, path, JsonValueKind.Object);
        var typeName = RequireString(root, "type", path);
        var text = RequireProperty(root, "value", path, JsonValueKind.String).GetString()!;
        return typeName switch
        {
            "System.String" => text,
            "System.Char" when text.Length == 1 => text[0],
            "System.Boolean" => bool.Parse(text),
            "System.Byte" => byte.Parse(text, CultureInfo.InvariantCulture),
            "System.SByte" => sbyte.Parse(text, CultureInfo.InvariantCulture),
            "System.Int16" => short.Parse(text, CultureInfo.InvariantCulture),
            "System.UInt16" => ushort.Parse(text, CultureInfo.InvariantCulture),
            "System.Int32" => int.Parse(text, CultureInfo.InvariantCulture),
            "System.UInt32" => uint.Parse(text, CultureInfo.InvariantCulture),
            "System.Int64" => long.Parse(text, CultureInfo.InvariantCulture),
            "System.UInt64" => ulong.Parse(text, CultureInfo.InvariantCulture),
            "System.Single" => float.Parse(text, CultureInfo.InvariantCulture),
            "System.Double" => double.Parse(text, CultureInfo.InvariantCulture),
            "System.Decimal" => decimal.Parse(text, CultureInfo.InvariantCulture),
            "System.Byte[]" => Convert.FromHexString(text),
            "System.DateTime" => DateTime.Parse(text, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind),
            "System.DateTimeOffset" => DateTimeOffset.Parse(text, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind),
            "System.TimeSpan" => TimeSpan.ParseExact(text, "c", CultureInfo.InvariantCulture),
            "System.Guid" => Guid.ParseExact(text, "D"),
            _ => ReadEnumValue(typeName, text, path)
        };
    }

    private static object ReadEnumValue(string typeName, string text, string path)
    {
        var type = Type.GetType(typeName)
            ?? AppDomain.CurrentDomain.GetAssemblies()
                .Select(assembly => assembly.GetType(typeName))
                .FirstOrDefault(static candidate => candidate is not null);
        if (type is null || !type.IsEnum)
        {
            throw new InvalidDataException(path + " uses unsupported portable value type '" + typeName + "'.");
        }

        return Enum.Parse(type, text, ignoreCase: false);
    }

    private static IReadOnlyList<object>? ReadPortableValues(JsonElement root, string path)
    {
        RequireKind(root, path, JsonValueKind.Array);
        return root.EnumerateArray()
            .Select((value, index) => ReadPortableValue(value, path + "[" + index.ToString(CultureInfo.InvariantCulture) + "]")
                ?? throw new InvalidDataException(path + " cannot contain null values."))
            .ToArray();
    }

    private static IReadOnlyList<string> ReadStrings(JsonElement root, string path)
    {
        return root.EnumerateArray()
            .Select((value, index) => value.ValueKind == JsonValueKind.String
                ? value.GetString()!
                : throw new InvalidDataException(path + "[" + index.ToString(CultureInfo.InvariantCulture) + "] must be a string."))
            .ToArray();
    }

    private static TEnum ReadEnum<TEnum>(string value, string path)
        where TEnum : struct, Enum
    {
        if (!Enum.TryParse<TEnum>(value, ignoreCase: false, out var parsed) || !Enum.IsDefined(parsed))
        {
            throw new InvalidDataException(path + " uses unsupported value '" + value + "'.");
        }

        return parsed;
    }

    private static string? ReadNullableString(JsonElement root, string propertyName, string path)
    {
        if (!root.TryGetProperty(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        RequireKind(value, path + "." + propertyName, JsonValueKind.String);
        return value.GetString();
    }

    private static long? ReadNullableInt64(JsonElement root, string propertyName, string path)
    {
        if (!root.TryGetProperty(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (value.ValueKind != JsonValueKind.Number || !value.TryGetInt64(out var parsed))
        {
            throw new InvalidDataException(path + "." + propertyName + " must be an integer or null.");
        }

        return parsed;
    }

    private static int? ReadNullableInt32(JsonElement root, string propertyName, string path)
    {
        if (!root.TryGetProperty(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (value.ValueKind != JsonValueKind.Number || !value.TryGetInt32(out var parsed))
        {
            throw new InvalidDataException(path + "." + propertyName + " must be an integer or null.");
        }

        return parsed;
    }

    private static DateTimeOffset? ReadNullableTimestamp(JsonElement root, string propertyName, string path)
    {
        if (!root.TryGetProperty(propertyName, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (value.ValueKind != JsonValueKind.String || !value.TryGetDateTimeOffset(out var parsed))
        {
            throw new InvalidDataException(path + "." + propertyName + " must be an ISO-8601 timestamp or null.");
        }

        return parsed;
    }

    private static string RequireString(JsonElement root, string propertyName, string path)
    {
        var value = RequireProperty(root, propertyName, path, JsonValueKind.String).GetString();
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new InvalidDataException(path + "." + propertyName + " cannot be empty.");
        }

        return value;
    }

    private static long RequireInt64(JsonElement root, string propertyName, string path)
    {
        var value = RequireProperty(root, propertyName, path, JsonValueKind.Number);
        if (!value.TryGetInt64(out var parsed))
        {
            throw new InvalidDataException(path + "." + propertyName + " must be an integer.");
        }

        return parsed;
    }

    private static DateTimeOffset RequireTimestamp(JsonElement root, string propertyName, string path)
    {
        var value = RequireProperty(root, propertyName, path, JsonValueKind.String);
        if (!value.TryGetDateTimeOffset(out var parsed))
        {
            throw new InvalidDataException(path + "." + propertyName + " must be an ISO-8601 timestamp.");
        }

        return parsed;
    }

    private static void RequireExactString(JsonElement root, string propertyName, string path, string expected)
    {
        var actual = RequireProperty(root, propertyName, path, JsonValueKind.String).GetString();
        if (!string.Equals(actual, expected, StringComparison.Ordinal))
        {
            throw new InvalidDataException(path + "." + propertyName + " must be '" + expected + "'.");
        }
    }

    private static void RequireExactInt32(JsonElement root, string propertyName, string path, int expected)
    {
        var value = RequireProperty(root, propertyName, path, JsonValueKind.Number);
        if (!value.TryGetInt32(out var actual) || actual != expected)
        {
            throw new InvalidDataException(path + "." + propertyName + " must be " + expected.ToString(CultureInfo.InvariantCulture) + ".");
        }
    }

    private static JsonElement RequireProperty(JsonElement root, string propertyName, string path, params JsonValueKind[] kinds)
    {
        if (!root.TryGetProperty(propertyName, out var property))
        {
            throw new InvalidDataException(path + " is missing required property '" + propertyName + "'.");
        }

        RequireKind(property, path + "." + propertyName, kinds);
        return property;
    }

    private static void RequireKind(JsonElement value, string path, params JsonValueKind[] kinds)
    {
        if (!kinds.Contains(value.ValueKind))
        {
            throw new InvalidDataException(path + " has unsupported JSON kind " + value.ValueKind + ".");
        }
    }

    private sealed record ComparisonPlanFilter(string Name, object? Value);
}

/// <summary>Couples a selector display name with its portable executable descriptor.</summary>
/// <param name="Name">The selector display name.</param>
/// <param name="Descriptor">The executable selector descriptor.</param>
public sealed record ComparisonPlanSelectorDocument(string Name, ComparisonSelectorDescriptor Descriptor)
{
    internal static ComparisonPlanSelectorDocument FromSelector(ComparisonSelector selector)
    {
        if (selector.Descriptor is null)
        {
            throw new ComparisonExportException(
                "Comparison plan contains runtime-only selectors and cannot be exported as portable data.",
                []);
        }

        return new ComparisonPlanSelectorDocument(selector.Name, selector.Descriptor);
    }

    internal ComparisonSelector Compile(string path)
    {
        return CompileDescriptor(Descriptor, path + ".descriptor").WithName(Name);
    }

    private static ComparisonSelector CompileDescriptor(ComparisonSelectorDescriptor descriptor, string path)
    {
        return descriptor.Kind switch
        {
            "windowName" => ComparisonSelector.ForWindowName(RequireStringValue(descriptor.Value, path + ".value")),
            "key" => ComparisonSelector.ForKey(RequireValue(descriptor.Value, path + ".value")),
            "source" => ComparisonSelector.ForSource(RequireValue(descriptor.Value, path + ".value")),
            "sources" => ComparisonSelector.ForSources(RequireValues(descriptor.Values, path + ".values")),
            "cohort" => ComparisonSelector.ForCohortSources(
                RequireValues(descriptor.Values, path + ".values"),
                CompileActivity(descriptor, path)),
            "partition" => ComparisonSelector.ForPartition(RequireValue(descriptor.Value, path + ".value")),
            "positionRange" => ComparisonSelector.ForPositionRange(
                descriptor.StartPosition ?? throw new InvalidDataException(path + ".startPosition is required."),
                descriptor.EndPosition),
            "timeRange" => ComparisonSelector.ForTimeRange(
                descriptor.StartTime ?? throw new InvalidDataException(path + ".startTime is required."),
                descriptor.EndTime,
                descriptor.Clock),
            "and" => CompileComposite(descriptor, path, static (left, right) => left.And(right)),
            "or" => CompileComposite(descriptor, path, static (left, right) => left.Or(right)),
            _ => throw new InvalidDataException(path + " uses unsupported selector kind '" + descriptor.Kind + "'.")
        };
    }

    private static ComparisonSelector CompileComposite(
        ComparisonSelectorDescriptor descriptor,
        string path,
        Func<ComparisonSelector, ComparisonSelector, ComparisonSelector> combine)
    {
        if (descriptor.Children.Count != 2)
        {
            throw new InvalidDataException(path + " requires exactly two child descriptors.");
        }

        var left = CompileDescriptor(descriptor.Children[0], path + ".children[0]");
        var right = CompileDescriptor(descriptor.Children[1], path + ".children[1]");

        try
        {
            return combine(left, right);
        }
        catch (InvalidOperationException exception)
        {
            throw new InvalidDataException(path + " has invalid cohort composition: " + exception.Message, exception);
        }
    }

    private static CohortActivity CompileActivity(ComparisonSelectorDescriptor descriptor, string path)
    {
        return descriptor.Activity switch
        {
            "any" => CohortActivity.Any(),
            "all" => CohortActivity.All(),
            "none" => CohortActivity.None(),
            "at-least" when descriptor.Count.HasValue => CohortActivity.AtLeast(descriptor.Count.Value),
            "at-most" when descriptor.Count.HasValue => CohortActivity.AtMost(descriptor.Count.Value),
            "exactly" when descriptor.Count.HasValue => CohortActivity.Exactly(descriptor.Count.Value),
            _ => throw new InvalidDataException(path + " uses unsupported cohort activity '" + descriptor.Activity + "'.")
        };
    }

    private static object RequireValue(object? value, string path)
    {
        return value ?? throw new InvalidDataException(path + " is required.");
    }

    private static string RequireStringValue(object? value, string path)
    {
        return value as string ?? throw new InvalidDataException(path + " must be a string.");
    }

    private static IReadOnlyList<object> RequireValues(IReadOnlyList<object> values, string path)
    {
        if (values.Count == 0)
        {
            throw new InvalidDataException(path + " must contain at least one value.");
        }

        return values;
    }
}
