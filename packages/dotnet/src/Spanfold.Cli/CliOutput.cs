using System.Text.Json;
using System.Text.Json.Serialization;

namespace Spanfold.Cli;

internal static class CliOutput
{
    private static readonly JsonSerializerOptions JsonOutputOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter() }
    };

    internal static void WriteJson<T>(TextWriter writer, T value)
    {
        writer.Write(JsonSerializer.Serialize(value, JsonOutputOptions));
    }

    internal static void WriteError(TextWriter writer, string message)
    {
        writer.Write("{\"error\":");
        writer.Write(JsonSerializer.Serialize(message));
        writer.Write('}');
    }
}
