using System.Diagnostics.CodeAnalysis;
using System.Globalization;

namespace Spanfold.Comparison;

internal enum ComparisonComparatorKind
{
    Unknown = 0,
    Overlap,
    Residual,
    Missing,
    Coverage,
    Gap,
    SymmetricDifference,
    Containment
}

internal abstract record ComparisonComparatorDeclaration
{
    internal sealed record BuiltIn(ComparisonComparatorKind Kind) : ComparisonComparatorDeclaration;

    internal sealed record LeadLag(
        LeadLagTransition Transition,
        TemporalAxis Axis,
        long ToleranceMagnitude) : ComparisonComparatorDeclaration;

    internal sealed record AsOf(
        AsOfDirection Direction,
        TemporalAxis Axis,
        long ToleranceMagnitude) : ComparisonComparatorDeclaration;
}

internal static class ComparisonComparatorDeclarationParser
{
    private static readonly BuiltInDefinition[] BuiltIns =
    [
        new("overlap", ComparisonComparatorKind.Overlap),
        new("residual", ComparisonComparatorKind.Residual),
        new("missing", ComparisonComparatorKind.Missing),
        new("coverage", ComparisonComparatorKind.Coverage),
        new("gap", ComparisonComparatorKind.Gap),
        new("symmetric-difference", ComparisonComparatorKind.SymmetricDifference),
        new("containment", ComparisonComparatorKind.Containment)
    ];

    internal static IReadOnlyList<string> BuiltInDeclarations { get; } =
        Array.AsReadOnly(BuiltIns.Select(static definition => definition.Declaration).ToArray());

    internal static bool TryParse(
        string declaration,
        [NotNullWhen(true)] out ComparisonComparatorDeclaration? parsed)
    {
        for (var i = 0; i < BuiltIns.Length; i++)
        {
            var builtIn = BuiltIns[i];
            if (string.Equals(builtIn.Declaration, declaration, StringComparison.Ordinal))
            {
                parsed = new ComparisonComparatorDeclaration.BuiltIn(builtIn.Kind);
                return true;
            }
        }

        var parts = declaration.Split(':');
        if (parts.Length != 4)
        {
            parsed = null;
            return false;
        }

        if (TryParseLeadLag(parts, out parsed))
        {
            return true;
        }

        return TryParseAsOf(parts, out parsed);
    }

    private static bool TryParseLeadLag(
        string[] parts,
        [NotNullWhen(true)] out ComparisonComparatorDeclaration? parsed)
    {
        if (!string.Equals(parts[0], "lead-lag", StringComparison.Ordinal)
            || !Enum.TryParse(parts[1], ignoreCase: false, out LeadLagTransition transition)
            || !TryParseAxisAndTolerance(parts, out var axis, out var toleranceMagnitude))
        {
            parsed = null;
            return false;
        }

        parsed = new ComparisonComparatorDeclaration.LeadLag(transition, axis, toleranceMagnitude);
        return true;
    }

    private static bool TryParseAsOf(
        string[] parts,
        [NotNullWhen(true)] out ComparisonComparatorDeclaration? parsed)
    {
        if (!string.Equals(parts[0], "asof", StringComparison.Ordinal)
            || !Enum.TryParse(parts[1], ignoreCase: false, out AsOfDirection direction)
            || !TryParseAxisAndTolerance(parts, out var axis, out var toleranceMagnitude))
        {
            parsed = null;
            return false;
        }

        parsed = new ComparisonComparatorDeclaration.AsOf(direction, axis, toleranceMagnitude);
        return true;
    }

    private static bool TryParseAxisAndTolerance(
        string[] parts,
        out TemporalAxis axis,
        out long toleranceMagnitude)
    {
        toleranceMagnitude = default;

        if (!Enum.TryParse(parts[2], ignoreCase: false, out axis)
            || axis == TemporalAxis.Unknown)
        {
            return false;
        }

        return long.TryParse(
            parts[3],
            NumberStyles.Integer,
            CultureInfo.InvariantCulture,
            out toleranceMagnitude)
            && toleranceMagnitude >= 0;
    }

    private readonly record struct BuiltInDefinition(
        string Declaration,
        ComparisonComparatorKind Kind);
}
