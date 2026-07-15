namespace Spanfold.Internal.Analysis;

internal static class TemporalMagnitudeMath
{
    internal static long SaturatingAdd(long value, long nonNegativeMagnitude)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(nonNegativeMagnitude);

        return value > long.MaxValue - nonNegativeMagnitude
            ? long.MaxValue
            : value + nonNegativeMagnitude;
    }

    internal static long SaturatingSubtract(long left, long right)
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
}
