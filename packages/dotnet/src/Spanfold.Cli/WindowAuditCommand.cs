using System.Globalization;

namespace Spanfold.Cli;

internal static class WindowAuditCommand
{
    internal static int Execute(string[] args, TextWriter stdout)
    {
        var result = ExecuteComparison(args);
        var bundle = AuditBundleWriter.Write(CliArguments.ReadRequiredOption(args, "--out"), result);
        CliOutput.WriteJson(stdout, bundle.Manifest);
        return result.IsValid ? 0 : 1;
    }

    private static ComparisonResult ExecuteComparison(string[] args)
    {
        var target = CliArguments.ReadRequiredOption(args, "--target");
        var againstSources = CliArguments.ReadOptionValues(args, "--against");
        if (againstSources.Count == 0)
        {
            throw new ArgumentException("The audit-windows command requires --against <source>.");
        }

        var windowName = CliArguments.ReadOptionalOption(args, "--window");
        var comparators = CliArguments.ReadOptionValues(args, "--comparators");
        if (comparators.Count == 0)
        {
            comparators = ["overlap", "residual", "coverage"];
        }

        var history = WindowHistoryJsonLines.Read(args[1], windowName);
        var comparisonName = CliArguments.ReadOptionalOption(args, "--name") ?? "Spanfold Window Audit";
        var builder = history.Compare(comparisonName)
            .Target(target, selector => selector.Source(target));

        foreach (var source in againstSources)
        {
            builder.Against(source, selector => selector.Source(source));
        }

        var scope = string.IsNullOrWhiteSpace(windowName)
            ? ComparisonScope.All()
            : ComparisonScope.Window(windowName);

        builder = builder
            .Within(_ => scope)
            .Using(_ => BuildComparators(comparators));

        if (CliArguments.HasFlag(args, "--strict"))
        {
            builder = builder.Strict();
        }

        var horizon = CliArguments.ReadOptionalOption(args, "--live-horizon-position");
        return horizon is null
            ? builder.Run()
            : builder.RunLive(TemporalPoint.ForPosition(long.Parse(horizon, CultureInfo.InvariantCulture)));
    }

    private static ComparisonComparatorBuilder BuildComparators(IReadOnlyList<string> comparators)
    {
        var builder = new ComparisonComparatorBuilder();
        for (var index = 0; index < comparators.Count; index++)
        {
            builder.Declaration(comparators[index]);
        }

        return builder;
    }
}
