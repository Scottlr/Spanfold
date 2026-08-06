namespace Spanfold.Cli;

internal static class CliArguments
{
    internal static void ValidateOptions(string[] args, string command)
    {
        var valueOptions = string.Equals(command, "audit-windows", StringComparison.Ordinal)
            ? new HashSet<string>(StringComparer.Ordinal)
            {
                "--target", "--against", "--out", "--window", "--comparators",
                "--name", "--live-horizon-position"
            }
            : new HashSet<string>(StringComparer.Ordinal) { "--format", "--out", "--spec", "--suite" };

        var flags = new HashSet<string>(StringComparer.Ordinal) { "--strict" };
        for (var index = 2; index < args.Length; index++)
        {
            var option = args[index];
            if (!option.StartsWith("--", StringComparison.Ordinal))
            {
                throw new ArgumentException("Unexpected positional argument: " + option);
            }

            if (!valueOptions.Contains(option) && !flags.Contains(option))
            {
                throw new ArgumentException("Unknown option: " + option);
            }

            if (!valueOptions.Contains(option))
            {
                continue;
            }

            if (index + 1 >= args.Length || args[index + 1].StartsWith("--", StringComparison.Ordinal))
            {
                throw new ArgumentException("Option " + option + " requires a value.");
            }

            index++;
        }
    }

    internal static string? ReadOptionalOption(string[] args, string optionName)
    {
        for (var index = 2; index < args.Length - 1; index++)
        {
            if (string.Equals(args[index], optionName, StringComparison.Ordinal))
            {
                return string.IsNullOrWhiteSpace(args[index + 1]) ? null : args[index + 1];
            }
        }

        return null;
    }

    internal static string ReadRequiredOption(string[] args, string optionName)
    {
        for (var index = 2; index < args.Length - 1; index++)
        {
            if (string.Equals(args[index], optionName, StringComparison.Ordinal))
            {
                ArgumentException.ThrowIfNullOrWhiteSpace(args[index + 1]);
                return args[index + 1];
            }
        }

        throw new ArgumentException("The command requires " + optionName + " <value>.");
    }

    internal static IReadOnlyList<string> ReadOptionValues(string[] args, string optionName)
    {
        var values = new List<string>();
        for (var index = 2; index < args.Length - 1; index++)
        {
            if (!string.Equals(args[index], optionName, StringComparison.Ordinal))
            {
                continue;
            }

            values.AddRange(args[index + 1]
                .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Where(static value => value.Length > 0));
        }

        return values;
    }

    internal static bool HasFlag(string[] args, string flag)
    {
        for (var index = 2; index < args.Length; index++)
        {
            if (string.Equals(args[index], flag, StringComparison.Ordinal))
            {
                return true;
            }
        }

        return false;
    }
}
