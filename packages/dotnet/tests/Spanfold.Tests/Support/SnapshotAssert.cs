using System.Runtime.CompilerServices;
using System.Text;
using System.Text.RegularExpressions;

namespace Spanfold.Tests.Support;

internal static partial class SnapshotAssert
{
    internal static void Match(
        string snapshotName,
        string actual,
        SnapshotNormalization normalization = SnapshotNormalization.Default,
        [CallerFilePath] string callerFilePath = "")
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(snapshotName);
        ArgumentNullException.ThrowIfNull(actual);
        ArgumentException.ThrowIfNullOrWhiteSpace(callerFilePath);

        var snapshotPath = Path.Combine(
            Path.GetDirectoryName(callerFilePath)!,
            "Snapshots",
            snapshotName + ".snap");
        var expected = File.ReadAllText(snapshotPath);

        Equal(expected, actual, snapshotPath, normalization);
    }

    internal static void Equal(
        string expected,
        string actual,
        string snapshotPath = "<inline>",
        SnapshotNormalization normalization = SnapshotNormalization.Default)
    {
        var normalizedExpected = Normalize(expected, normalization);
        var normalizedActual = Normalize(actual, normalization);

        if (string.Equals(normalizedExpected, normalizedActual, StringComparison.Ordinal))
        {
            return;
        }

        throw new InvalidOperationException(BuildDiffMessage(snapshotPath, normalizedExpected, normalizedActual));
    }

    private static string Normalize(string value, SnapshotNormalization normalization)
    {
        var normalized = value.Replace("\r\n", "\n", StringComparison.Ordinal)
            .Replace('\r', '\n')
            .TrimEnd();

        if ((normalization & SnapshotNormalization.RowIds) == SnapshotNormalization.RowIds)
        {
            normalized = NormalizeRowIds(normalized);
        }

        if ((normalization & SnapshotNormalization.RecordIds) == SnapshotNormalization.RecordIds)
        {
            normalized = NormalizeRecordIds(normalized);
        }

        return normalized + "\n";
    }

    private static string NormalizeRecordIds(string value)
    {
        var next = 1;
        var ids = new Dictionary<string, string>(StringComparer.Ordinal);

        return RecordIdRegex().Replace(value, match =>
        {
            if (!ids.TryGetValue(match.Value, out var replacement))
            {
                replacement = "<record-id:" + next.ToString(System.Globalization.CultureInfo.InvariantCulture) + ">";
                ids.Add(match.Value, replacement);
                next++;
            }

            return replacement;
        });
    }

    private static string NormalizeRowIds(string value)
    {
        var nextByType = new Dictionary<string, int>(StringComparer.Ordinal);
        var replacements = new Dictionary<string, string>(StringComparer.Ordinal);
        return RowIdRegex().Replace(value, match =>
        {
            if (!replacements.TryGetValue(match.Value, out var replacement))
            {
                var type = match.Groups["type"].Value;
                nextByType.TryGetValue(type, out var next);
                replacement = type + "[" + next.ToString(System.Globalization.CultureInfo.InvariantCulture) + "]";
                nextByType[type] = next + 1;
                replacements.Add(match.Value, replacement);
            }

            return replacement;
        });
    }

    private static string BuildDiffMessage(string snapshotPath, string expected, string actual)
    {
        var expectedLines = expected.Split('\n');
        var actualLines = actual.Split('\n');
        var max = Math.Max(expectedLines.Length, actualLines.Length);
        var firstDifference = 0;

        while (firstDifference < max)
        {
            var expectedLine = firstDifference < expectedLines.Length ? expectedLines[firstDifference] : "<missing>";
            var actualLine = firstDifference < actualLines.Length ? actualLines[firstDifference] : "<missing>";
            if (!string.Equals(expectedLine, actualLine, StringComparison.Ordinal))
            {
                break;
            }

            firstDifference++;
        }

        var start = Math.Max(0, firstDifference - 2);
        var end = Math.Min(max - 1, firstDifference + 2);
        var builder = new StringBuilder();
        builder.AppendLine("Snapshot mismatch: " + snapshotPath);
        builder.AppendLine("First difference at line " + (firstDifference + 1).ToString(System.Globalization.CultureInfo.InvariantCulture) + ".");
        builder.AppendLine();

        for (var i = start; i <= end; i++)
        {
            var expectedLine = i < expectedLines.Length ? expectedLines[i] : "<missing>";
            var actualLine = i < actualLines.Length ? actualLines[i] : "<missing>";

            if (string.Equals(expectedLine, actualLine, StringComparison.Ordinal))
            {
                builder.AppendLine("  " + (i + 1).ToString(System.Globalization.CultureInfo.InvariantCulture) + ": " + expectedLine);
                continue;
            }

            builder.AppendLine("- " + (i + 1).ToString(System.Globalization.CultureInfo.InvariantCulture) + ": " + expectedLine);
            builder.AppendLine("+ " + (i + 1).ToString(System.Globalization.CultureInfo.InvariantCulture) + ": " + actualLine);
        }

        return builder.ToString();
    }

    [GeneratedRegex("[a-f0-9]{64}", RegexOptions.CultureInvariant)]
    private static partial Regex RecordIdRegex();

    [GeneratedRegex("(?<type>[A-Za-z][A-Za-z0-9]*):[a-f0-9]{64}", RegexOptions.CultureInvariant)]
    private static partial Regex RowIdRegex();
}
