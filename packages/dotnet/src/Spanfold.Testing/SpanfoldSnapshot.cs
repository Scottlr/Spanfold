using System.Text.RegularExpressions;

namespace Spanfold.Testing;

/// <summary>
/// Provides small snapshot helpers for Spanfold JSON and markdown artifacts.
/// </summary>
public static partial class SpanfoldSnapshot
{
    /// <summary>
    /// Normalizes line endings, trailing whitespace, and deterministic Spanfold record IDs.
    /// </summary>
    /// <param name="value">The snapshot text to normalize.</param>
    /// <param name="normalizeRecordIds">Whether known Spanfold record-ID fields should be replaced with stable placeholders.</param>
    /// <param name="normalizeUnlabeledHex">Whether every 64-character lowercase hexadecimal token should be normalized. This broad mode is opt-in.</param>
    /// <returns>The normalized snapshot text.</returns>
    public static string Normalize(
        string value,
        bool normalizeRecordIds = true,
        bool normalizeUnlabeledHex = false)
    {
        ArgumentNullException.ThrowIfNull(value);

        var normalized = value.Replace("\r\n", "\n", StringComparison.Ordinal)
            .Replace('\r', '\n')
            .TrimEnd();

        if (normalizeRecordIds)
        {
            normalized = NormalizeRecordIds(normalized, normalizeUnlabeledHex);
        }

        return normalized + "\n";
    }

    /// <summary>
    /// Asserts that two snapshot strings are equal after Spanfold normalization.
    /// </summary>
    /// <param name="expected">The expected snapshot.</param>
    /// <param name="actual">The actual snapshot.</param>
    /// <param name="normalizeUnlabeledHex">Whether every unlabeled 64-character lowercase hexadecimal token should be normalized.</param>
    /// <exception cref="SpanfoldAssertionException">Thrown when the normalized snapshots differ.</exception>
    public static void AssertEqual(
        string expected,
        string actual,
        bool normalizeUnlabeledHex = false)
    {
        var normalizedExpected = Normalize(expected, normalizeUnlabeledHex: normalizeUnlabeledHex);
        var normalizedActual = Normalize(actual, normalizeUnlabeledHex: normalizeUnlabeledHex);
        if (string.Equals(normalizedExpected, normalizedActual, StringComparison.Ordinal))
        {
            return;
        }

        throw new SpanfoldAssertionException("Spanfold snapshot mismatch." + Environment.NewLine + BuildDiff(normalizedExpected, normalizedActual));
    }

    private static string NormalizeRecordIds(string value, bool normalizeUnlabeledHex)
    {
        var next = 1;
        var ids = new Dictionary<string, string>(StringComparer.Ordinal);

        var normalized = KnownRecordIdRegex().Replace(value, match =>
        {
            var id = match.Groups["id"].Value;
            if (!ids.TryGetValue(id, out var replacement))
            {
                replacement = "<record-id:" + next.ToString(System.Globalization.CultureInfo.InvariantCulture) + ">";
                ids.Add(id, replacement);
                next++;
            }

            return match.Value.Replace(id, replacement, StringComparison.Ordinal);
        });

        return normalizeUnlabeledHex
            ? NormalizeAllRecordIds(normalized, ids, next, out _)
            : normalized;
    }

    private static string NormalizeAllRecordIds(
        string value,
        Dictionary<string, string> ids,
        int next,
        out int updatedNext)
    {
        var current = next;
        var normalized = RecordIdRegex().Replace(value, match =>
        {
            if (!ids.TryGetValue(match.Value, out var replacement))
            {
                replacement = "<record-id:" + current.ToString(System.Globalization.CultureInfo.InvariantCulture) + ">";
                ids.Add(match.Value, replacement);
                current++;
            }

            return replacement;
        });

        updatedNext = current;
        return normalized;
    }

    private static string BuildDiff(string expected, string actual)
    {
        var expectedLines = expected.Split('\n');
        var actualLines = actual.Split('\n');
        var max = Math.Max(expectedLines.Length, actualLines.Length);

        for (var i = 0; i < max; i++)
        {
            var expectedLine = i < expectedLines.Length ? expectedLines[i] : "<missing>";
            var actualLine = i < actualLines.Length ? actualLines[i] : "<missing>";
            if (!string.Equals(expectedLine, actualLine, StringComparison.Ordinal))
            {
                return "First difference at line " + (i + 1).ToString(System.Globalization.CultureInfo.InvariantCulture) + ".";
            }
        }

        return "Snapshots differ.";
    }

    [GeneratedRegex(@"\b[a-f0-9]{64}\b", RegexOptions.CultureInvariant)]
    private static partial Regex RecordIdRegex();

    [GeneratedRegex(@"(?:record=|""recordId""\s*:\s*"")(?<id>\b[a-f0-9]{64}\b)", RegexOptions.CultureInvariant)]
    private static partial Regex KnownRecordIdRegex();
}
