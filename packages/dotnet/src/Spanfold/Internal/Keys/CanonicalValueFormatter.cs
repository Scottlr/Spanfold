using System.Globalization;

namespace Spanfold.Internal.Keys;

internal static class CanonicalValueFormatter
{
    internal static string Format(object? value)
    {
        return value switch
        {
            null => "<null>",
            string text => typeof(string).FullName + ":" + text,
            char character => typeof(char).FullName + ":" + character,
            bool boolean => typeof(bool).FullName + ":" + (boolean ? "true" : "false"),
            byte[] bytes => typeof(byte[]).FullName + ":" + Convert.ToHexString(bytes),
            DateTime dateTime => typeof(DateTime).FullName + ":" + dateTime.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
            DateTimeOffset dateTimeOffset => typeof(DateTimeOffset).FullName + ":" + dateTimeOffset.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
            TimeSpan timeSpan => typeof(TimeSpan).FullName + ":" + timeSpan.ToString("c", CultureInfo.InvariantCulture),
            Guid guid => typeof(Guid).FullName + ":" + guid.ToString("D"),
            Enum enumValue => enumValue.GetType().FullName + ":" + enumValue.ToString(),
            byte number => FormatNumber(typeof(byte), number),
            sbyte number => FormatNumber(typeof(sbyte), number),
            short number => FormatNumber(typeof(short), number),
            ushort number => FormatNumber(typeof(ushort), number),
            int number => FormatNumber(typeof(int), number),
            uint number => FormatNumber(typeof(uint), number),
            long number => FormatNumber(typeof(long), number),
            ulong number => FormatNumber(typeof(ulong), number),
            float number => FormatNumber(typeof(float), number),
            double number => FormatNumber(typeof(double), number),
            decimal number => FormatNumber(typeof(decimal), number),
            _ => throw new ArgumentException(
                $"Value type '{value.GetType().FullName}' is not supported as a stable identity. "
                + "Use a canonical scalar value such as string, numeric, Guid, or timestamp.",
                nameof(value))
        };
    }

    private static string FormatNumber(Type type, IFormattable value) =>
        type.FullName + ":" + value.ToString(null, CultureInfo.InvariantCulture);
}
