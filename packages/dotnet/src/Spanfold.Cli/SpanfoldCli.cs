using System.Text.Json;

namespace Spanfold.Cli;

internal static class SpanfoldCli
{
    public static int Run(string[] args, TextWriter stdout, TextWriter stderr)
    {
        ArgumentNullException.ThrowIfNull(args);
        ArgumentNullException.ThrowIfNull(stdout);
        ArgumentNullException.ThrowIfNull(stderr);

        try
        {
            if (args.Length < 2)
            {
                CliOutput.WriteError(stderr, "Usage: spanfold <validate-plan|compare|explain|audit|check|suite> <fixture.json> [options], spanfold episodes <plan.json> <windows.jsonl> [--format json|markdown], spanfold verify-bundle <directory>, or spanfold diff <baseline> <current>.");
                return 2;
            }

            var command = args[0];
            if (!IsKnownCommand(command))
            {
                CliOutput.WriteError(stderr, "Unknown command: " + command);
                return 2;
            }

            return Dispatch(command, args, stdout);
        }
        catch (Exception exception) when (
            exception is IOException
                or JsonException
                or ArgumentException
                or KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            CliOutput.WriteError(stderr, exception.Message);
            return 2;
        }
    }

    private static int Dispatch(string command, string[] args, TextWriter stdout)
    {
        if (string.Equals(command, "verify-bundle", StringComparison.Ordinal))
        {
            return ArtifactCommands.VerifyBundle(args, stdout);
        }

        if (string.Equals(command, "diff", StringComparison.Ordinal))
        {
            return ArtifactCommands.Diff(args, stdout);
        }

        if (string.Equals(command, "episodes", StringComparison.Ordinal))
        {
            return EpisodesCommand.Execute(args, stdout);
        }

        CliArguments.ValidateOptions(args, command);

        if (string.Equals(command, "audit-windows", StringComparison.Ordinal))
        {
            return WindowAuditCommand.Execute(args, stdout);
        }

        return FixtureCommands.Execute(command, args, stdout);
    }

    private static bool IsKnownCommand(string command)
    {
        return string.Equals(command, "validate-plan", StringComparison.Ordinal)
            || string.Equals(command, "compare", StringComparison.Ordinal)
            || string.Equals(command, "explain", StringComparison.Ordinal)
            || string.Equals(command, "audit", StringComparison.Ordinal)
            || string.Equals(command, "audit-windows", StringComparison.Ordinal)
            || string.Equals(command, "check", StringComparison.Ordinal)
            || string.Equals(command, "suite", StringComparison.Ordinal)
            || string.Equals(command, "verify-bundle", StringComparison.Ordinal)
            || string.Equals(command, "episodes", StringComparison.Ordinal)
            || string.Equals(command, "diff", StringComparison.Ordinal);
    }
}
