using System.Globalization;
using System.Text;
using System.Text.Json;

using Spanfold.Artifacts.Episodes;
using Spanfold.Episodes;

namespace Spanfold.Artifacts.Internal;

internal static class EpisodeAnalysisExporter
{
    private const string ResultSchema = "spanfold.episode.analysis.result";
    private const int SchemaVersion = 1;
    private static readonly Encoding PortableUtf8 = new UTF8Encoding(false, true);

    internal static string ExportJson(EpisodeAnalysisResultDocument document)
    {
        using var stream = new MemoryStream();
        using (var writer = new Utf8JsonWriter(stream, new JsonWriterOptions { Indented = true }))
        {
            WriteJson(writer, document);
        }

        var json = Encoding.UTF8.GetString(stream.ToArray());
        return CanonicalizeJsonStrings(json);
    }

    internal static string ExportMarkdown(EpisodeAnalysisResultDocument document)
    {
        var definition = document.Document;
        var result = document.Result;
        var targetEpisodes = OrderEpisodes(result.TargetEpisodes);
        var againstEpisodes = OrderEpisodes(result.AgainstEpisodes);
        var relations = OrderRelations(result, targetEpisodes, againstEpisodes);
        var text = new StringBuilder();
        text.Append("# Episode analysis: ").AppendLine(definition.Name).AppendLine();
        AppendFact(text, "Window", definition.WindowName);
        AppendFact(text, "Normalization axis", AxisName(definition.NormalizationAxis));
        AppendFact(text, "Stitch tolerance", Format(definition.StitchTolerance));
        AppendFact(text, "Relation tolerance", Format(definition.RelationTolerance));
        AppendFact(text, "Evaluation horizon", Optional(definition.LiveHorizon));
        text.AppendLine();

        var summary = result.Summary;
        text.AppendLine("## Summary").AppendLine();
        text.AppendLine("| Target episodes | Against episodes | Matched target | Matched against | Unmatched target | Unmatched against | One-to-one | Splits | Merges | Complex | Total overlap |");
        text.AppendLine("| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
        text.Append("| ").Append(Format(summary.TargetEpisodeCount))
            .Append(" | ").Append(Format(summary.AgainstEpisodeCount))
            .Append(" | ").Append(Format(summary.MatchedTargetEpisodeCount))
            .Append(" | ").Append(Format(summary.MatchedAgainstEpisodeCount))
            .Append(" | ").Append(Format(summary.UnmatchedTargetEpisodeCount))
            .Append(" | ").Append(Format(summary.UnmatchedAgainstEpisodeCount))
            .Append(" | ").Append(Format(summary.OneToOneRelationCount))
            .Append(" | ").Append(Format(summary.SplitRelationCount))
            .Append(" | ").Append(Format(summary.MergeRelationCount))
            .Append(" | ").Append(Format(summary.ComplexRelationCount))
            .Append(" | ").Append(Format(summary.TotalOverlapMagnitude)).AppendLine(" |").AppendLine();

        AppendEpisodes(text, "Target", definition.Target, targetEpisodes);
        AppendEpisodes(text, "Against", definition.Against, againstEpisodes);
        AppendRelations(text, relations);
        return text.ToString();
    }

    private static void WriteJson(Utf8JsonWriter writer, EpisodeAnalysisResultDocument document)
    {
        var definition = document.Document;
        var result = document.Result;
        var targetEpisodes = OrderEpisodes(result.TargetEpisodes);
        var againstEpisodes = OrderEpisodes(result.AgainstEpisodes);
        var relations = OrderRelations(result, targetEpisodes, againstEpisodes);
        writer.WriteStartObject();
        writer.WriteString("schema", ResultSchema);
        writer.WriteNumber("schemaVersion", SchemaVersion);
        writer.WriteString("analysisName", definition.Name);
        writer.WriteString("windowName", definition.WindowName);
        writer.WriteString("normalizationAxis", AxisName(definition.NormalizationAxis));
        writer.WriteNumber("stitchTolerance", definition.StitchTolerance);
        writer.WriteNumber("relationTolerance", definition.RelationTolerance);
        WriteOptionalNumber(writer, "evaluationHorizon", definition.LiveHorizon);
        WriteSide(writer, "target", definition.Target, result.TargetEpisodes, targetEpisodes);
        WriteSide(writer, "against", definition.Against, result.AgainstEpisodes, againstEpisodes);
        WriteSummary(writer, result.Summary);
        WriteRelations(writer, relations);
        writer.WriteEndObject();
    }

    private static void WriteSide(
        Utf8JsonWriter writer,
        string propertyName,
        EpisodeAnalysisSource source,
        EpisodeSet set,
        IReadOnlyList<Episode> episodes)
    {
        writer.WriteStartObject(propertyName);
        writer.WriteString("name", source.Name);
        writer.WriteString("source", source.Source);
        writer.WriteStartObject("summary");
        writer.WriteNumber("episodeCount", set.Summary.EpisodeCount);
        writer.WriteNumber("finalEpisodeCount", set.Summary.FinalEpisodeCount);
        writer.WriteNumber("provisionalEpisodeCount", set.Summary.ProvisionalEpisodeCount);
        writer.WriteNumber("totalActiveMagnitude", set.Summary.TotalActiveMagnitude);
        writer.WriteNumber("totalElapsedMagnitude", set.Summary.TotalElapsedMagnitude);
        writer.WriteNumber("totalInternalGapMagnitude", set.Summary.TotalInternalGapMagnitude);
        writer.WriteEndObject();
        writer.WriteStartArray("episodes");
        for (var index = 0; index < episodes.Count; index++)
        {
            WriteEpisode(writer, index, episodes[index]);
        }

        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static void WriteEpisode(Utf8JsonWriter writer, int index, Episode episode)
    {
        writer.WriteStartObject();
        writer.WriteNumber("index", index);
        writer.WriteString("key", (string)episode.Key);
        writer.WriteString("partition", (string?)episode.Partition);
        writer.WriteNumber("start", PointMagnitude(episode.Envelope.Start));
        writer.WriteNumber("end", PointMagnitude(episode.Envelope.End!.Value));
        writer.WriteNumber("fragmentCount", episode.Fragments.Count);
        writer.WriteNumber("activeMagnitude", episode.ActiveMagnitude);
        writer.WriteNumber("elapsedMagnitude", episode.ElapsedMagnitude);
        writer.WriteNumber("internalGapMagnitude", episode.InternalGapMagnitude);
        writer.WriteString("finality", FinalityName(episode.Finality));
        writer.WriteEndObject();
    }

    private static void WriteSummary(Utf8JsonWriter writer, EpisodeComparisonSummary summary)
    {
        writer.WriteStartObject("summary");
        writer.WriteNumber("targetEpisodeCount", summary.TargetEpisodeCount);
        writer.WriteNumber("againstEpisodeCount", summary.AgainstEpisodeCount);
        writer.WriteNumber("matchedTargetEpisodeCount", summary.MatchedTargetEpisodeCount);
        writer.WriteNumber("matchedAgainstEpisodeCount", summary.MatchedAgainstEpisodeCount);
        writer.WriteNumber("unmatchedTargetEpisodeCount", summary.UnmatchedTargetEpisodeCount);
        writer.WriteNumber("unmatchedAgainstEpisodeCount", summary.UnmatchedAgainstEpisodeCount);
        writer.WriteNumber("oneToOneRelationCount", summary.OneToOneRelationCount);
        writer.WriteNumber("splitRelationCount", summary.SplitRelationCount);
        writer.WriteNumber("mergeRelationCount", summary.MergeRelationCount);
        writer.WriteNumber("complexRelationCount", summary.ComplexRelationCount);
        writer.WriteNumber("totalOverlapMagnitude", summary.TotalOverlapMagnitude);
        writer.WriteEndObject();
    }

    private static void WriteRelations(
        Utf8JsonWriter writer,
        IReadOnlyList<PortableRelation> relations)
    {
        writer.WriteStartArray("relations");
        for (var index = 0; index < relations.Count; index++)
        {
            var portable = relations[index];
            var relation = portable.Relation;
            writer.WriteStartObject();
            writer.WriteString("kind", RelationKindName(relation.Kind));
            WriteEpisodeIndexes(writer, "targetEpisodeIndexes", portable.TargetIndexes);
            WriteEpisodeIndexes(writer, "againstEpisodeIndexes", portable.AgainstIndexes);
            writer.WriteString("finality", FinalityName(relation.Finality));
            writer.WriteNumber("overlapMagnitude", relation.Metrics.OverlapMagnitude);
            WriteOptionalNumber(writer, "minimumGapMagnitude", relation.Metrics.MinimumGapMagnitude);
            WriteOptionalNumber(writer, "onsetDeltaMagnitude", relation.Metrics.OnsetDeltaMagnitude);
            WriteOptionalNumber(writer, "recoveryDeltaMagnitude", relation.Metrics.RecoveryDeltaMagnitude);
            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static void WriteEpisodeIndexes(
        Utf8JsonWriter writer,
        string propertyName,
        IReadOnlyList<int> indexes)
    {
        writer.WriteStartArray(propertyName);
        for (var index = 0; index < indexes.Count; index++)
        {
            writer.WriteNumberValue(indexes[index]);
        }

        writer.WriteEndArray();
    }

    private static int FindEpisodeIndex(IReadOnlyList<Episode> episodes, Episode episode)
    {
        for (var index = 0; index < episodes.Count; index++)
        {
            if (episodes[index].Id == episode.Id)
            {
                return index;
            }
        }

        throw new InvalidOperationException("Episode relation references an episode outside its materialized side.");
    }

    private static void AppendEpisodes(
        StringBuilder text,
        string label,
        EpisodeAnalysisSource source,
        IReadOnlyList<Episode> episodes)
    {
        text.Append("## ").Append(label).Append(" episodes: ").AppendLine(source.Name).AppendLine();
        text.Append("Source: `").Append(EscapeCode(source.Source)).AppendLine("`").AppendLine();
        text.AppendLine("| Index | Key | Partition | Start | End | Fragments | Active | Elapsed | Internal gap | Finality |");
        text.AppendLine("| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |");
        for (var index = 0; index < episodes.Count; index++)
        {
            var episode = episodes[index];
            text.Append("| ").Append(Format(index))
                .Append(" | ").Append(EscapeCell(IdentityJsonLiteral((string)episode.Key)))
                .Append(" | ").Append(EscapeCell(IdentityJsonLiteral((string?)episode.Partition)))
                .Append(" | ").Append(Format(PointMagnitude(episode.Envelope.Start)))
                .Append(" | ").Append(Format(PointMagnitude(episode.Envelope.End!.Value)))
                .Append(" | ").Append(Format(episode.Fragments.Count))
                .Append(" | ").Append(Format(episode.ActiveMagnitude))
                .Append(" | ").Append(Format(episode.ElapsedMagnitude))
                .Append(" | ").Append(Format(episode.InternalGapMagnitude))
                .Append(" | ").Append(FinalityName(episode.Finality)).AppendLine(" |");
        }

        text.AppendLine();
    }

    private static void AppendRelations(
        StringBuilder text,
        IReadOnlyList<PortableRelation> relations)
    {
        text.AppendLine("## Relations").AppendLine();
        text.AppendLine("| Kind | Target indexes | Against indexes | Finality | Overlap | Minimum gap | Onset delta | Recovery delta |");
        text.AppendLine("| --- | --- | --- | --- | ---: | ---: | ---: | ---: |");
        for (var index = 0; index < relations.Count; index++)
        {
            var portable = relations[index];
            var relation = portable.Relation;
            text.Append("| ").Append(RelationKindName(relation.Kind))
                .Append(" | ").Append(JoinIndexes(portable.TargetIndexes))
                .Append(" | ").Append(JoinIndexes(portable.AgainstIndexes))
                .Append(" | ").Append(FinalityName(relation.Finality))
                .Append(" | ").Append(Format(relation.Metrics.OverlapMagnitude))
                .Append(" | ").Append(Optional(relation.Metrics.MinimumGapMagnitude))
                .Append(" | ").Append(Optional(relation.Metrics.OnsetDeltaMagnitude))
                .Append(" | ").Append(Optional(relation.Metrics.RecoveryDeltaMagnitude)).AppendLine(" |");
        }

        text.AppendLine();
    }

    private static string JoinIndexes(IReadOnlyList<int> indexes)
    {
        return string.Join(", ", indexes.Select(index => Format(index)));
    }

    private static void AppendFact(StringBuilder text, string label, string value)
    {
        text.Append("- ").Append(label).Append(": `").Append(EscapeCode(value)).AppendLine("`");
    }

    private static void WriteOptionalNumber(Utf8JsonWriter writer, string propertyName, long? value)
    {
        if (value.HasValue)
        {
            writer.WriteNumber(propertyName, value.Value);
        }
        else
        {
            writer.WriteNull(propertyName);
        }
    }

    private static long PointMagnitude(TemporalPoint point)
    {
        return point.Axis == TemporalAxis.ProcessingPosition ? point.Position : point.Timestamp.Ticks;
    }

    private static string AxisName(TemporalAxis axis)
    {
        return axis == TemporalAxis.ProcessingPosition ? "processingPosition" : "timestamp";
    }

    private static string FinalityName(ComparisonFinality finality)
    {
        return finality == ComparisonFinality.Final ? "final" : "provisional";
    }

    private static string RelationKindName(EpisodeRelationKind kind)
    {
        return kind switch
        {
            EpisodeRelationKind.OneToOne => "oneToOne",
            EpisodeRelationKind.Split => "split",
            EpisodeRelationKind.Merge => "merge",
            EpisodeRelationKind.Complex => "complex",
            EpisodeRelationKind.UnmatchedTarget => "unmatchedTarget",
            EpisodeRelationKind.UnmatchedAgainst => "unmatchedAgainst",
            _ => throw new ArgumentOutOfRangeException(nameof(kind))
        };
    }

    private static IReadOnlyList<Episode> OrderEpisodes(EpisodeSet set)
    {
        var episodes = set.Episodes.ToArray();
        Array.Sort(episodes, PortableEpisodeComparer.Instance);
        return episodes;
    }

    private static IReadOnlyList<PortableRelation> OrderRelations(
        EpisodeComparisonResult result,
        IReadOnlyList<Episode> targetEpisodes,
        IReadOnlyList<Episode> againstEpisodes)
    {
        var relations = result.Relations
            .Select(relation => new PortableRelation(
                relation,
                relation.TargetEpisodes
                    .Select(episode => FindEpisodeIndex(targetEpisodes, episode))
                    .Order()
                    .ToArray(),
                relation.AgainstEpisodes
                    .Select(episode => FindEpisodeIndex(againstEpisodes, episode))
                    .Order()
                    .ToArray()))
            .ToArray();
        Array.Sort(relations, PortableRelationComparer.Instance);
        return relations;
    }

    private static string IdentityJsonLiteral(string? value)
    {
        if (value is null)
        {
            return "null";
        }

        _ = PortableUtf8.GetByteCount(value);
        var literal = new StringBuilder(value.Length + 2).Append('"');
        for (var index = 0; index < value.Length; index++)
        {
            var character = value[index];
            switch (character)
            {
                case '"':
                    literal.Append("\\\"");
                    break;
                case '\\':
                    literal.Append("\\\\");
                    break;
                case '\b':
                    literal.Append("\\b");
                    break;
                case '\f':
                    literal.Append("\\f");
                    break;
                case '\n':
                    literal.Append("\\n");
                    break;
                case '\r':
                    literal.Append("\\r");
                    break;
                case '\t':
                    literal.Append("\\t");
                    break;
                default:
                    if (character < ' ')
                    {
                        literal.Append("\\u").Append(((int)character).ToString("x4", CultureInfo.InvariantCulture));
                    }
                    else
                    {
                        literal.Append(character);
                    }

                    break;
            }
        }

        return literal.Append('"').ToString();
    }

    private static string CanonicalizeJsonStrings(string json)
    {
        var canonical = new StringBuilder(json.Length);
        var isInsideString = false;
        for (var index = 0; index < json.Length; index++)
        {
            var character = json[index];
            if (!isInsideString)
            {
                canonical.Append(character);
                isInsideString = character == '"';
                continue;
            }

            if (character == '"')
            {
                canonical.Append(character);
                isInsideString = false;
                continue;
            }

            if (character != '\\')
            {
                canonical.Append(character);
                continue;
            }

            var escape = json[++index];
            switch (escape)
            {
                case '"':
                    canonical.Append("\\\"");
                    break;
                case '\\':
                    canonical.Append("\\\\");
                    break;
                case '/':
                    canonical.Append('/');
                    break;
                case 'b':
                    canonical.Append("\\b");
                    break;
                case 'f':
                    canonical.Append("\\f");
                    break;
                case 'n':
                    canonical.Append("\\n");
                    break;
                case 'r':
                    canonical.Append("\\r");
                    break;
                case 't':
                    canonical.Append("\\t");
                    break;
                case 'u':
                    AppendCanonicalEscapedScalar(canonical, json, ref index);
                    break;
                default:
                    throw new InvalidOperationException("The JSON writer produced an unsupported string escape.");
            }
        }

        return canonical.ToString();
    }

    private static void AppendCanonicalEscapedScalar(StringBuilder canonical, string json, ref int escapeIndex)
    {
        var codeUnit = ReadHexCodeUnit(json, escapeIndex + 1);
        escapeIndex += 4;
        if (!char.IsHighSurrogate(codeUnit))
        {
            AppendCanonicalScalar(canonical, codeUnit);
            return;
        }

        var lowEscapeIndex = escapeIndex + 1;
        if (json[lowEscapeIndex] != '\\' || json[lowEscapeIndex + 1] != 'u')
        {
            throw new InvalidOperationException("The JSON writer produced an unpaired high surrogate escape.");
        }

        var lowSurrogate = ReadHexCodeUnit(json, lowEscapeIndex + 2);
        if (!char.IsLowSurrogate(lowSurrogate))
        {
            throw new InvalidOperationException("The JSON writer produced an unpaired high surrogate escape.");
        }

        canonical.Append(codeUnit).Append(lowSurrogate);
        escapeIndex = lowEscapeIndex + 5;
    }

    private static char ReadHexCodeUnit(string json, int startIndex)
    {
        return (char)int.Parse(json.AsSpan(startIndex, 4), NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture);
    }

    private static void AppendCanonicalScalar(StringBuilder canonical, char character)
    {
        switch (character)
        {
            case '"':
                canonical.Append("\\\"");
                break;
            case '\\':
                canonical.Append("\\\\");
                break;
            case '\b':
                canonical.Append("\\b");
                break;
            case '\f':
                canonical.Append("\\f");
                break;
            case '\n':
                canonical.Append("\\n");
                break;
            case '\r':
                canonical.Append("\\r");
                break;
            case '\t':
                canonical.Append("\\t");
                break;
            default:
                if (character < ' ')
                {
                    canonical.Append("\\u").Append(((int)character).ToString("x4", CultureInfo.InvariantCulture));
                }
                else
                {
                    canonical.Append(character);
                }

                break;
        }
    }

    private static string EscapeCell(string value)
    {
        return value.Replace("\\", "\\\\", StringComparison.Ordinal)
            .Replace("|", "\\|", StringComparison.Ordinal)
            .Replace("\r", " ", StringComparison.Ordinal)
            .Replace("\n", " ", StringComparison.Ordinal);
    }

    private static string EscapeCode(string value)
    {
        return value.Replace("`", "\\`", StringComparison.Ordinal);
    }

    private static string Optional(long? value)
    {
        return value.HasValue ? Format(value.Value) : "-";
    }

    private static string Format(long value)
    {
        return value.ToString(CultureInfo.InvariantCulture);
    }

    private sealed record PortableRelation(
        EpisodeRelation Relation,
        IReadOnlyList<int> TargetIndexes,
        IReadOnlyList<int> AgainstIndexes);

    private sealed class PortableEpisodeComparer : IComparer<Episode>
    {
        internal static PortableEpisodeComparer Instance { get; } = new();

        public int Compare(Episode? left, Episode? right)
        {
            if (ReferenceEquals(left, right))
            {
                return 0;
            }

            if (left is null)
            {
                return -1;
            }

            if (right is null)
            {
                return 1;
            }

            var comparison = CompareUtf8((string)left.Key, (string)right.Key);
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = CompareNullableUtf8((string?)left.Partition, (string?)right.Partition);
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = PointMagnitude(left.Envelope.Start).CompareTo(PointMagnitude(right.Envelope.Start));
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = PointMagnitude(left.Envelope.End!.Value).CompareTo(PointMagnitude(right.Envelope.End!.Value));
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = left.Fragments.Count.CompareTo(right.Fragments.Count);
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = left.ActiveMagnitude.CompareTo(right.ActiveMagnitude);
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = left.ElapsedMagnitude.CompareTo(right.ElapsedMagnitude);
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = left.InternalGapMagnitude.CompareTo(right.InternalGapMagnitude);
            return comparison != 0
                ? comparison
                : FinalityRank(left.Finality).CompareTo(FinalityRank(right.Finality));
        }
    }

    private sealed class PortableRelationComparer : IComparer<PortableRelation>
    {
        internal static PortableRelationComparer Instance { get; } = new();

        public int Compare(PortableRelation? left, PortableRelation? right)
        {
            if (ReferenceEquals(left, right))
            {
                return 0;
            }

            if (left is null)
            {
                return -1;
            }

            if (right is null)
            {
                return 1;
            }

            var comparison = CompareIndexes(left.TargetIndexes, right.TargetIndexes);
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = CompareIndexes(left.AgainstIndexes, right.AgainstIndexes);
            if (comparison != 0)
            {
                return comparison;
            }

            comparison = string.CompareOrdinal(
                RelationKindName(left.Relation.Kind),
                RelationKindName(right.Relation.Kind));
            if (comparison != 0)
            {
                return comparison;
            }

            return FinalityRank(left.Relation.Finality).CompareTo(FinalityRank(right.Relation.Finality));
        }
    }

    private static int CompareUtf8(string left, string right)
    {
        return PortableUtf8.GetBytes(left).AsSpan().SequenceCompareTo(PortableUtf8.GetBytes(right));
    }

    private static int CompareNullableUtf8(string? left, string? right)
    {
        if (left is null)
        {
            return right is null ? 0 : -1;
        }

        return right is null ? 1 : CompareUtf8(left, right);
    }

    private static int CompareIndexes(IReadOnlyList<int> left, IReadOnlyList<int> right)
    {
        var count = Math.Min(left.Count, right.Count);
        for (var index = 0; index < count; index++)
        {
            var comparison = left[index].CompareTo(right[index]);
            if (comparison != 0)
            {
                return comparison;
            }
        }

        return left.Count.CompareTo(right.Count);
    }

    private static int FinalityRank(ComparisonFinality finality)
    {
        return finality == ComparisonFinality.Final ? 0 : 1;
    }
}
