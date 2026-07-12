using System.Security.Cryptography;
using System.Text;
using System.Globalization;
using Spanfold.Internal.Keys;

namespace Spanfold.Internal.Comparison;

internal static class ComparisonRowIdentity
{
    internal static string Create(string rowType, object row)
    {
        ArgumentNullException.ThrowIfNull(row);
        var payload = rowType + "\n" + Describe(row);
        var hash = SHA256.HashData(Encoding.UTF8.GetBytes(payload));
        return rowType + ":" + Convert.ToHexString(hash).ToLowerInvariant();
    }

    private static string Describe(object row)
    {
        var builder = new StringBuilder();
        switch (row)
        {
            case OverlapRow value:
                AppendCommon(builder, value.WindowName, value.Key, value.Partition, value.Range);
                AppendIds(builder, value.TargetRecordIds, value.AgainstRecordIds);
                break;
            case ResidualRow value:
                AppendCommon(builder, value.WindowName, value.Key, value.Partition, value.Range);
                AppendIds(builder, value.TargetRecordIds);
                break;
            case MissingRow value:
                AppendCommon(builder, value.WindowName, value.Key, value.Partition, value.Range);
                AppendIds(builder, value.AgainstRecordIds);
                break;
            case CoverageRow value:
                AppendCommon(builder, value.WindowName, value.Key, value.Partition, value.Range);
                builder.Append(value.TargetMagnitude.ToString("R", CultureInfo.InvariantCulture)).Append('|')
                    .Append(value.CoveredMagnitude.ToString("R", CultureInfo.InvariantCulture));
                AppendIds(builder, value.TargetRecordIds, value.AgainstRecordIds);
                break;
            case GapRow value:
                AppendCommon(builder, value.WindowName, value.Key, value.Partition, value.Range);
                break;
            case SymmetricDifferenceRow value:
                AppendCommon(builder, value.WindowName, value.Key, value.Partition, value.Range);
                builder.Append(value.Side).Append('|');
                AppendIds(builder, value.TargetRecordIds, value.AgainstRecordIds);
                break;
            case ContainmentRow value:
                AppendCommon(builder, value.WindowName, value.Key, value.Partition, value.Range);
                builder.Append(value.Status).Append('|');
                AppendIds(builder, value.TargetRecordIds, value.ContainerRecordIds);
                break;
            case LeadLagRow value:
                AppendScope(builder, value.WindowName, value.Key, value.Partition);
                builder.Append(value.Transition).Append('|');
                AppendPoint(builder, value.TargetPoint);
                AppendPoint(builder, value.ComparisonPoint);
                AppendNullableId(builder, value.TargetRecordId);
                AppendNullableId(builder, value.ComparisonRecordId);
                break;
            case AsOfRow value:
                AppendScope(builder, value.WindowName, value.Key, value.Partition);
                builder.Append(value.Direction).Append('|').Append(value.Status).Append('|');
                AppendPoint(builder, value.TargetPoint);
                AppendPoint(builder, value.MatchedPoint);
                AppendNullableId(builder, value.TargetRecordId);
                AppendNullableId(builder, value.MatchedRecordId);
                break;
            default:
                throw new ArgumentException("Unsupported comparison row type.", nameof(row));
        }

        return builder.ToString();
    }

    private static void AppendCommon(StringBuilder builder, string windowName, object key, object? partition, TemporalRange range)
    {
        AppendScope(builder, windowName, key, partition);
        builder.Append(range.Axis).Append('|').Append(range.EndStatus).Append('|');
        AppendPoint(builder, range.Start);
        AppendPoint(builder, range.End);
    }

    private static void AppendScope(StringBuilder builder, string windowName, object key, object? partition)
    {
        builder.Append(windowName.Length).Append(':').Append(windowName).Append('|')
            .Append(CanonicalValueFormatter.Format(key)).Append('|')
            .Append(CanonicalValueFormatter.Format(partition)).Append('|');
    }

    private static void AppendPoint(StringBuilder builder, TemporalPoint? point)
    {
        if (!point.HasValue)
        {
            builder.Append("<null>|");
            return;
        }

        var value = point.Value;
        builder.Append(value.Axis).Append(':');
        if (value.Axis == TemporalAxis.ProcessingPosition)
        {
            builder.Append(value.Position);
        }
        else
        {
            builder.Append(value.Timestamp.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture));
        }
        builder.Append('|');
    }

    private static void AppendIds(StringBuilder builder, params IReadOnlyList<WindowRecordId>[] groups)
    {
        foreach (var group in groups)
        {
            foreach (var id in group.OrderBy(static value => value.Value, StringComparer.Ordinal))
            {
                builder.Append(id.Value).Append(',');
            }
            builder.Append('|');
        }
    }

    private static void AppendNullableId(StringBuilder builder, WindowRecordId? id) =>
        builder.Append(id?.Value ?? "<null>").Append('|');
}
