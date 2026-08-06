using System.Globalization;
using System.Text.Json;

namespace Spanfold.Cli;

internal static class WindowHistoryJsonLines
{
    internal static WindowHistory Read(string path, string? defaultWindowName)
    {
        var builder = new ImportBuilder();
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
            var windowName = ReadOptionalString(window, "windowName") ?? defaultWindowName;
            if (string.IsNullOrWhiteSpace(windowName))
            {
                throw new ArgumentException("windows.jsonl line " + lineNumber.ToString(CultureInfo.InvariantCulture) + " must include windowName or use --window.");
            }

            AddWindow(builder, window, windowName);
        }

        return builder.Build();
    }

    private static void AddWindow(ImportBuilder builder, JsonElement window, string windowName)
    {
        var key = RequireProperty(window, "key", "$", JsonValueKind.String).GetString()!;
        var source = RequireProperty(window, "source", "$", JsonValueKind.String).GetString();
        var startPosition = RequireProperty(window, "startPosition", "$", JsonValueKind.Number).GetInt64();
        var partition = ReadOptionalString(window, "partition");
        var segments = ReadSegments(window);
        var tags = ReadTags(window);
        if (!window.TryGetProperty("endPosition", out var endPosition)
            || endPosition.ValueKind == JsonValueKind.Null)
        {
            builder.AddOpenWindow(windowName, key, startPosition, source, partition, segments, tags);
            return;
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
                ReadOptionalString(segment, "parentName")));
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

    private static string? ReadOptionalString(JsonElement element, string propertyName)
    {
        if (!element.TryGetProperty(propertyName, out var property)
            || property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        return property.GetString();
    }

    private static object? ReadPrimitive(JsonElement value)
    {
        return value.ValueKind switch
        {
            JsonValueKind.String => value.GetString(),
            JsonValueKind.Number => ReadNumber(value),
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            JsonValueKind.Null => null,
            _ => throw new ArgumentException("Fixture values must be string, number, boolean, or null.")
        };
    }

    private static object ReadNumber(JsonElement value)
    {
        if (value.TryGetInt64(out var longValue))
        {
            return longValue;
        }

        return value.GetDouble();
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
        for (var index = 0; index < expectedKinds.Length; index++)
        {
            if (element.ValueKind == expectedKinds[index])
            {
                return;
            }
        }

        throw new ArgumentException(path + " has unsupported JSON kind " + element.ValueKind + ".");
    }

    private sealed class ImportBuilder
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
            closed.Add(new ClosedWindow(
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
            open.Add(new OpenWindow(
                windowName,
                key,
                startPosition,
                source,
                partition,
                Segments: segments,
                Tags: tags));
        }

        internal WindowHistory Build() => WindowHistory.FromRecords(closed, open);
    }
}
