using System.Text.Json;

using Spanfold.Testing;

namespace Spanfold.Cli;

internal static class FixtureCommands
{
    internal static int Execute(string command, string[] args, TextWriter stdout)
    {
        var format = ReadFormat(args);
        using var fixture = JsonDocument.Parse(File.ReadAllText(args[1]));
        var result = ContractFixtureRunner.Run(fixture.RootElement);

        if (string.Equals(command, "audit", StringComparison.Ordinal))
        {
            var bundle = AuditBundleWriter.Write(CliArguments.ReadRequiredOption(args, "--out"), result);
            CliOutput.WriteJson(stdout, bundle.Manifest);
            return result.IsValid ? 0 : 1;
        }

        if (string.Equals(command, "check", StringComparison.Ordinal))
        {
            var specification = AssessmentDocument.ReadSpecification(CliArguments.ReadRequiredOption(args, "--spec"));
            var assessment = result.Assess(specification);
            CliOutput.WriteJson(stdout, assessment);
            return assessment.Passed ? 0 : 1;
        }

        if (string.Equals(command, "suite", StringComparison.Ordinal))
        {
            var suite = AssessmentDocument.ReadSuite(CliArguments.ReadRequiredOption(args, "--suite")).Evaluate(result);
            CliOutput.WriteJson(stdout, suite);
            return suite.Passed ? 0 : 1;
        }

        if (string.Equals(command, "validate-plan", StringComparison.Ordinal))
        {
            WriteDiagnostics(stdout, result);
            return result.IsValid ? 0 : 1;
        }

        if (string.Equals(command, "compare", StringComparison.Ordinal))
        {
            stdout.Write(format switch
            {
                "markdown" => result.ExportMarkdown(),
                "llm-context" => result.ExportLlmContext(),
                _ => result.ExportJson()
            });
            return result.IsValid ? 0 : 1;
        }

        if (string.Equals(command, "explain", StringComparison.Ordinal))
        {
            stdout.Write(result.ExportMarkdown());
            return result.IsValid ? 0 : 1;
        }

        return 2;
    }

    private static string ReadFormat(string[] args)
    {
        for (var index = 2; index < args.Length - 1; index++)
        {
            if (!string.Equals(args[index], "--format", StringComparison.Ordinal))
            {
                continue;
            }

            var format = args[index + 1];
            if (string.Equals(format, "json", StringComparison.Ordinal)
                || string.Equals(format, "markdown", StringComparison.Ordinal)
                || string.Equals(format, "llm-context", StringComparison.Ordinal))
            {
                return format;
            }

            throw new ArgumentException("Unsupported format: " + format);
        }

        return "json";
    }

    private static void WriteDiagnostics(TextWriter writer, ComparisonResult result)
    {
        writer.Write("{\"isValid\":");
        writer.Write(result.IsValid ? "true" : "false");
        writer.Write(",\"diagnostics\":[");
        for (var index = 0; index < result.Diagnostics.Count; index++)
        {
            if (index > 0)
            {
                writer.Write(',');
            }

            writer.Write('"');
            writer.Write(result.Diagnostics[index].Code.ToString());
            writer.Write('"');
        }

        writer.Write("]}");
    }
}
