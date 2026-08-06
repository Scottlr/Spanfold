using Spanfold.Artifacts.Episodes;

namespace Spanfold.Cli;

internal static class EpisodesCommand
{
    internal static int Execute(string[] args, TextWriter stdout)
    {
        if (args.Length < 3)
        {
            throw new ArgumentException("The episodes command requires <plan.json> <windows.jsonl>.");
        }

        ValidateOptions(args);

        var format = ReadFormat(args);
        var document = EpisodeAnalysisDocument.Read(args[1]);
        var history = WindowHistoryJsonLines.Read(args[2], document.WindowName);
        var result = document.Execute(history);
        stdout.Write(string.Equals(format, "markdown", StringComparison.Ordinal)
            ? result.ExportMarkdown()
            : result.ExportJson());
        return 0;
    }

    private static void ValidateOptions(string[] args)
    {
        for (var index = 3; index < args.Length; index++)
        {
            if (!string.Equals(args[index], "--format", StringComparison.Ordinal))
            {
                throw new ArgumentException("Unknown option: " + args[index]);
            }

            if (index + 1 >= args.Length || args[index + 1].StartsWith("--", StringComparison.Ordinal))
            {
                throw new ArgumentException("Option --format requires a value.");
            }

            index++;
        }
    }

    private static string ReadFormat(string[] args)
    {
        for (var index = 3; index < args.Length - 1; index++)
        {
            if (!string.Equals(args[index], "--format", StringComparison.Ordinal))
            {
                continue;
            }

            var format = args[index + 1];
            if (string.Equals(format, "json", StringComparison.Ordinal)
                || string.Equals(format, "markdown", StringComparison.Ordinal))
            {
                return format;
            }

            throw new ArgumentException("Unsupported Episode format: " + format);
        }

        return "json";
    }
}
