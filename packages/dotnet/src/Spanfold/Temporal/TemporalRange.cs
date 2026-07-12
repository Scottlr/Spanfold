namespace Spanfold;

/// <summary>
/// Represents a half-open temporal range used by Spanfold analysis.
/// </summary>
/// <remarks>
/// Temporal ranges use half-open semantics: the start point is included and
/// the end point is excluded. A range with the same start and end is empty.
/// Open ranges must be clipped to an effective end before duration or overlap
/// calculations can be performed.
/// </remarks>
public readonly record struct TemporalRange
{
    private TemporalRange(
        TemporalPoint start,
        TemporalPoint? end,
        TemporalRangeEndStatus endStatus)
    {
        if (end.HasValue)
        {
            EnsureComparable(start, end.Value);

            if (end.Value.CompareTo(start) < 0)
            {
                throw new ArgumentException("Temporal range end cannot be earlier than the start.", nameof(end));
            }
        }

        Start = start;
        End = end;
        EndStatus = endStatus;
    }

    /// <summary>
    /// Gets the inclusive start of the range.
    /// </summary>
    public TemporalPoint Start { get; }

    /// <summary>
    /// Gets the exclusive end of the range, when one has been established.
    /// </summary>
    public TemporalPoint? End { get; }

    /// <summary>
    /// Gets how the end of the range was determined.
    /// </summary>
    public TemporalRangeEndStatus EndStatus { get; }

    /// <summary>
    /// Gets the temporal axis shared by the range bounds.
    /// </summary>
    public TemporalAxis Axis => Start.Axis;

    /// <summary>
    /// Gets whether the range has an effective end point.
    /// </summary>
    public bool HasEnd => End.HasValue;

    /// <summary>
    /// Gets whether the range represents a recorded closed window.
    /// </summary>
    public bool IsClosed => EndStatus == TemporalRangeEndStatus.Closed;

    /// <summary>
    /// Gets whether the range has the same start and end point.
    /// </summary>
    /// <exception cref="InvalidOperationException">
    /// Thrown when the range does not have an effective end.
    /// </exception>
    public bool IsEmpty => RequireEnd().CompareTo(Start) == 0;

    /// <summary>
    /// Creates a closed half-open range.
    /// </summary>
    /// <param name="start">The inclusive range start.</param>
    /// <param name="end">The exclusive range end.</param>
    /// <returns>A closed temporal range.</returns>
    public static TemporalRange Closed(TemporalPoint start, TemporalPoint end)
    {
        return new TemporalRange(start, end, TemporalRangeEndStatus.Closed);
    }

    /// <summary>
    /// Creates an open range with no effective end.
    /// </summary>
    /// <param name="start">The inclusive range start.</param>
    /// <returns>An open temporal range.</returns>
    public static TemporalRange Open(TemporalPoint start)
    {
        return new TemporalRange(start, end: null, TemporalRangeEndStatus.UnknownEnd);
    }

    /// <summary>
    /// Creates a range whose effective end was produced by analysis policy.
    /// </summary>
    /// <param name="start">The inclusive range start.</param>
    /// <param name="end">The exclusive effective range end.</param>
    /// <param name="endStatus">The policy that produced the effective end.</param>
    /// <returns>A temporal range with an effective end.</returns>
    /// <exception cref="ArgumentException">
    /// Thrown when <paramref name="endStatus" /> does not describe a policy
    /// end.
    /// </exception>
    public static TemporalRange WithEffectiveEnd(
        TemporalPoint start,
        TemporalPoint end,
        TemporalRangeEndStatus endStatus)
    {
        if (endStatus is TemporalRangeEndStatus.Unknown
            or TemporalRangeEndStatus.UnknownEnd
            or TemporalRangeEndStatus.Closed)
        {
            throw new ArgumentException("Effective ranges must use a clipping or horizon end status.", nameof(endStatus));
        }

        return new TemporalRange(start, end, endStatus);
    }

    /// <summary>
    /// Gets the length of a processing-position range.
    /// </summary>
    /// <returns>The number of processing positions covered by the range.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown when the range is not on the processing-position axis or does not
    /// have an effective end.
    /// </exception>
    public long GetPositionLength()
    {
        var end = RequireEnd();

        if (Axis != TemporalAxis.ProcessingPosition)
        {
            throw new InvalidOperationException("Only processing-position ranges expose a position length.");
        }

        return SaturatingSubtract(end.Position, Start.Position);
    }

    /// <summary>
    /// Gets the duration of a timestamp range.
    /// </summary>
    /// <returns>The timestamp duration covered by the range.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown when the range is not on the timestamp axis or does not have an
    /// effective end.
    /// </exception>
    public TimeSpan GetTimeDuration()
    {
        var end = RequireEnd();

        if (Axis != TemporalAxis.Timestamp)
        {
            throw new InvalidOperationException("Only timestamp ranges expose a time duration.");
        }

        return TimeSpan.FromTicks(SaturatingSubtract(end.Timestamp.Ticks, Start.Timestamp.Ticks));
    }

    private static long SaturatingSubtract(long left, long right)
    {
        if (right > 0 && left < long.MinValue + right)
        {
            return long.MinValue;
        }

        if (right < 0 && left > long.MaxValue + right)
        {
            return long.MaxValue;
        }

        return left - right;
    }

    /// <summary>
    /// Determines whether this range overlaps another half-open range.
    /// </summary>
    /// <param name="other">The other range to compare.</param>
    /// <returns><see langword="true" /> when the ranges overlap.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown when either range has no effective end.
    /// </exception>
    public bool Overlaps(TemporalRange other)
    {
        var thisEnd = RequireEnd();
        var otherEnd = other.RequireEnd();
        EnsureComparable(Start, other.Start);

        return Start.CompareTo(otherEnd) < 0
            && other.Start.CompareTo(thisEnd) < 0;
    }

    /// <summary>
    /// Determines whether this range contains a temporal point.
    /// </summary>
    /// <param name="point">The point to test.</param>
    /// <returns><see langword="true" /> when the point is inside the half-open range.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown when the range has no effective end.
    /// </exception>
    public bool Contains(TemporalPoint point)
    {
        var end = RequireEnd();
        EnsureComparable(Start, point);

        return Start.CompareTo(point) <= 0
            && point.CompareTo(end) < 0;
    }

    private TemporalPoint RequireEnd()
    {
        return End ?? throw new InvalidOperationException("Temporal ranges without an effective end cannot be used for duration or overlap calculations.");
    }

    private static void EnsureComparable(TemporalPoint first, TemporalPoint second)
    {
        _ = first.CompareTo(second);
    }
}
