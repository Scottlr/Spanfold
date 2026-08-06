using Spanfold;

namespace Spanfold.Internal.Keys;

internal sealed class SegmentContext : IEquatable<SegmentContext>
{
    private readonly WindowSegment[] segments;

    internal SegmentContext(IReadOnlyList<WindowSegment> segments) => this.segments = segments.ToArray();

    public bool Equals(SegmentContext? other)
    {
        if (other is null || this.segments.Length != other.segments.Length)
        {
            return false;
        }

        for (var i = 0; i < this.segments.Length; i++)
        {
            var left = this.segments[i];
            var right = other.segments[i];
            if (!string.Equals(left.Name, right.Name, StringComparison.Ordinal)
                || !string.Equals(left.ParentName, right.ParentName, StringComparison.Ordinal)
                || !EqualityComparer<object?>.Default.Equals(left.Value, right.Value))
            {
                return false;
            }
        }

        return true;
    }

    public override bool Equals(object? obj) => obj is SegmentContext other && this.Equals(other);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        foreach (var segment in this.segments)
        {
            hash.Add(segment.Name, StringComparer.Ordinal);
            hash.Add(segment.ParentName, StringComparer.Ordinal);
            hash.Add(segment.Value);
        }
        return hash.ToHashCode();
    }
}
