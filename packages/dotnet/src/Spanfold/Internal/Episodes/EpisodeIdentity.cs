using System.Globalization;
using System.Security.Cryptography;
using System.Text;

using Spanfold.Episodes;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Episodes;

internal static class EpisodeIdentity
{
    internal static EpisodeId Create(
        string windowName,
        object key,
        object? source,
        object? partition,
        TemporalAxis timeAxis,
        IReadOnlyList<EpisodeFragment> fragments)
    {
        var builder = new StringBuilder(capacity: 512);
        Append(builder, "schema", "spanfold-episode-v1");
        Append(builder, "window", windowName);
        Append(builder, "key", CanonicalValueFormatter.Format(key));
        Append(builder, "source", CanonicalValueFormatter.Format(source));
        Append(builder, "partition", CanonicalValueFormatter.Format(partition));
        Append(builder, "time-axis", timeAxis.ToString());
        Append(builder, "timestamp-clock", fragments[0].Range.Start.Clock ?? "<null>");
        Append(builder, "fragment-count", fragments.Count.ToString(CultureInfo.InvariantCulture));

        for (var i = 0; i < fragments.Count; i++)
        {
            var fragment = fragments[i];
            var end = fragment.Range.End
                ?? throw new InvalidOperationException("Episode fragments require an effective end.");
            Append(builder, "fragment-record-id", fragment.RecordId.Value);
            Append(builder, "fragment-start", FormatPoint(fragment.Range.Start));
            Append(builder, "fragment-end", FormatPoint(end));
            Append(builder, "fragment-end-status", fragment.Range.EndStatus.ToString());
            Append(builder, "fragment-finality", fragment.Finality.ToString());
        }

        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes(builder.ToString()));
        return new EpisodeId(Convert.ToHexString(bytes).ToLowerInvariant());
    }

    private static string FormatPoint(TemporalPoint point)
    {
        return point.Axis switch
        {
            TemporalAxis.ProcessingPosition => point.Position.ToString(CultureInfo.InvariantCulture),
            TemporalAxis.Timestamp => point.Timestamp.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
            _ => throw new InvalidOperationException("Episode identity requires a known temporal axis.")
        };
    }

    private static void Append(StringBuilder builder, string name, string value)
    {
        builder
            .Append(name)
            .Append('=')
            .Append(value.Length.ToString(CultureInfo.InvariantCulture))
            .Append(':')
            .Append(value)
            .Append(';');
    }
}
