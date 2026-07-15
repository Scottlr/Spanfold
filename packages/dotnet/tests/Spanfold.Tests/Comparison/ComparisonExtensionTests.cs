using System.Text.Json;

using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonExtensionTests
{
    [Fact]
    public void ExperimentalExtensionCanRegisterComparatorAndSelector()
    {
        var descriptor = ExperimentalQualityExtension.Describe();

        Assert.Equal("experimental-quality", descriptor.Id);
        Assert.Contains(descriptor.Comparators, comparator =>
            comparator.Declaration == "quality:drift");
        Assert.Contains(descriptor.Selectors, selector =>
            selector.Name == "region");
    }

    [Fact]
    public void CoreVocabularyRemainsDomainNeutral()
    {
        var coreTypeNames = typeof(ComparisonResult).Assembly
            .GetTypes()
            .Select(static type => type.FullName ?? string.Empty)
            .ToArray();

        Assert.DoesNotContain(coreTypeNames, name => name.Contains("ConsumerSpecific", StringComparison.Ordinal));
        Assert.DoesNotContain(coreTypeNames, name => name.Contains("DomainSpecific", StringComparison.Ordinal));
    }

    [Fact]
    public void ResultExportHandlesExtensionMetadata()
    {
        var result = new ComparisonResult(
            new ComparisonPlan(
                "Extension QA",
                ComparisonSelector.ForSource("provider-a"),
                [ComparisonSelector.ForSource("provider-b")],
                ComparisonScope.Window("DeviceOffline"),
                ComparisonNormalizationPolicy.Default,
                ["overlap"]
                ),
            [],
            extensionMetadata:
            [
                new ComparisonExtensionMetadata("experimental-quality", "driftThreshold", "0.025")
            ]);

        using var document = JsonDocument.Parse(result.ExportJson());
        var metadata = Assert.Single(document.RootElement.GetProperty("extensionMetadata").EnumerateArray());

        Assert.Equal("experimental-quality", metadata.GetProperty("extensionId").GetString());
        Assert.Equal("driftThreshold", metadata.GetProperty("key").GetString());
        Assert.Contains("extensionMetadata[0]: experimental-quality.driftThreshold=0.025", result.Explain());
    }

    private static class ExperimentalQualityExtension
    {
        internal static ComparisonExtensionDescriptor Describe()
        {
            return new ComparisonExtensionBuilder("experimental-quality", "Experimental Quality")
                .AddSelector("region", "Selects a region-scoped window.")
                .AddComparator("quality:drift", "Compares quality drift windows.")
                .AddMetadataKey("driftThreshold")
                .Build();
        }
    }
}
