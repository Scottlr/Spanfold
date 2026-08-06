using System.Security.Cryptography;
using System.Text;

using Spanfold.Internal.Keys;

namespace Spanfold.Comparison;

internal sealed class ComparisonPlanIdentity
{
    private const string Version = "spanfold.comparison.compatibility.v1";

    private ComparisonPlanIdentity(
        string fingerprint,
        IReadOnlyList<object> runtimeSelectorIdentities)
    {
        Fingerprint = fingerprint;
        this.runtimeSelectorIdentities = runtimeSelectorIdentities;
    }

    private readonly IReadOnlyList<object> runtimeSelectorIdentities;

    internal string Fingerprint { get; }

    internal static ComparisonPlanIdentity Create(ComparisonPlan plan)
    {
        ArgumentNullException.ThrowIfNull(plan);

        var canonical = new StringBuilder();
        var runtimeSelectorIdentities = new List<object>();
        Append(canonical, Version);
        AppendSelector(canonical, plan.Target, runtimeSelectorIdentities);
        Append(canonical, plan.Against.Count);
        for (var i = 0; i < plan.Against.Count; i++)
        {
            AppendSelector(canonical, plan.Against[i], runtimeSelectorIdentities);
        }

        AppendScope(canonical, plan.Scope);
        AppendNormalization(canonical, plan.Normalization);
        Append(canonical, plan.IsStrict);
        Append(canonical, plan.Comparators.Count);
        for (var i = 0; i < plan.Comparators.Count; i++)
        {
            Append(canonical, plan.Comparators[i]);
        }

        var fingerprint = Convert.ToHexString(
            SHA256.HashData(Encoding.UTF8.GetBytes(canonical.ToString())));
        return new ComparisonPlanIdentity(
            Version + ":" + fingerprint,
            Array.AsReadOnly(runtimeSelectorIdentities.ToArray()));
    }

    internal bool IsCompatibleWith(ComparisonPlanIdentity other)
    {
        ArgumentNullException.ThrowIfNull(other);
        if (!StringComparer.Ordinal.Equals(Fingerprint, other.Fingerprint)
            || this.runtimeSelectorIdentities.Count != other.runtimeSelectorIdentities.Count)
        {
            return false;
        }

        for (var i = 0; i < this.runtimeSelectorIdentities.Count; i++)
        {
            if (!ReferenceEquals(
                    this.runtimeSelectorIdentities[i],
                    other.runtimeSelectorIdentities[i]))
            {
                return false;
            }
        }

        return true;
    }

    private static void AppendSelector(
        StringBuilder canonical,
        ComparisonSelector? selector,
        List<object> runtimeSelectorIdentities)
    {
        if (!selector.HasValue)
        {
            Append(canonical, "missing");
            return;
        }

        var value = selector.Value;
        if (value.Descriptor is { } descriptor)
        {
            Append(canonical, "portable");
            AppendDescriptor(canonical, descriptor);
            return;
        }

        Append(canonical, "runtime");
        if (value.RuntimeIdentity is { } runtimeIdentity)
        {
            runtimeSelectorIdentities.Add(runtimeIdentity);
        }
    }

    private static void AppendDescriptor(
        StringBuilder canonical,
        ComparisonSelectorDescriptor descriptor)
    {
        Append(canonical, descriptor.Kind);
        Append(canonical, CanonicalValueFormatter.Format(descriptor.Value));
        Append(canonical, descriptor.Values.Count);
        for (var i = 0; i < descriptor.Values.Count; i++)
        {
            Append(canonical, CanonicalValueFormatter.Format(descriptor.Values[i]));
        }

        Append(canonical, descriptor.StartPosition);
        Append(canonical, descriptor.EndPosition);
        Append(canonical, descriptor.StartTime);
        Append(canonical, descriptor.EndTime);
        Append(canonical, descriptor.Clock);
        Append(canonical, descriptor.Activity);
        Append(canonical, descriptor.Count);
        Append(canonical, descriptor.Children.Count);
        for (var i = 0; i < descriptor.Children.Count; i++)
        {
            AppendDescriptor(canonical, descriptor.Children[i]);
        }
    }

    private static void AppendScope(StringBuilder canonical, ComparisonScope? scope)
    {
        if (scope is null)
        {
            Append(canonical, "missing");
            return;
        }

        Append(canonical, scope.WindowName);
        Append(canonical, scope.TimeAxis);
        Append(canonical, scope.SegmentFilters.Count);
        for (var i = 0; i < scope.SegmentFilters.Count; i++)
        {
            Append(canonical, scope.SegmentFilters[i].Name);
            Append(canonical, CanonicalValueFormatter.Format(scope.SegmentFilters[i].Value));
        }

        Append(canonical, scope.TagFilters.Count);
        for (var i = 0; i < scope.TagFilters.Count; i++)
        {
            Append(canonical, scope.TagFilters[i].Name);
            Append(canonical, CanonicalValueFormatter.Format(scope.TagFilters[i].Value));
        }
    }

    private static void AppendNormalization(
        StringBuilder canonical,
        ComparisonNormalizationPolicy normalization)
    {
        Append(canonical, normalization.TimeAxis);
        Append(canonical, normalization.OpenWindowPolicy);
        Append(canonical, normalization.NullTimestampPolicy);
    }

    private static void Append(StringBuilder canonical, object? value)
    {
        var text = value switch
        {
            null => string.Empty,
            DateTimeOffset timestamp => timestamp.ToUniversalTime().ToString("O"),
            _ => value.ToString() ?? string.Empty
        };

        canonical.Append(text.Length).Append(':').Append(text).Append(';');
    }
}
