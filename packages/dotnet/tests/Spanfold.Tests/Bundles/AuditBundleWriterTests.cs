using Spanfold.Testing;

namespace Spanfold.Tests.Bundles;

public sealed class AuditBundleWriterTests
{
    [Theory]
    [InlineData(-1)]
    [InlineData(2)]
    [InlineData(int.MaxValue)]
    public void Write_UndefinedProfile_DoesNotCreateDestinationParent(int profileValue)
    {
        var parent = Path.Combine(Path.GetTempPath(), "spanfold-tests-" + Guid.NewGuid().ToString("N"));
        var destination = Path.Combine(parent, "bundle");
        var options = new AuditBundleOptions { Profile = (ArtifactExportProfile)profileValue };

        try
        {
            var exception = Assert.Throws<ArgumentOutOfRangeException>(() =>
                AuditBundleWriter.Write(destination, CreateResult(), options: options));

            Assert.Equal(nameof(AuditBundleOptions.Profile), exception.ParamName);
            Assert.Equal((ArtifactExportProfile)profileValue, exception.ActualValue);
            Assert.False(Directory.Exists(parent));
        }
        finally
        {
            if (Directory.Exists(parent))
            {
                Directory.Delete(parent, recursive: true);
            }
        }
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(2)]
    [InlineData(int.MaxValue)]
    public void Write_UndefinedProfile_DoesNotAlterExistingDestination(int profileValue)
    {
        var parent = Directory.CreateTempSubdirectory("spanfold-tests-").FullName;
        var destination = Path.Combine(parent, "bundle");
        Directory.CreateDirectory(destination);
        var markerPath = Path.Combine(destination, "marker.txt");
        File.WriteAllText(markerPath, "preserve");
        var options = new AuditBundleOptions { Profile = (ArtifactExportProfile)profileValue };

        try
        {
            Assert.Throws<ArgumentOutOfRangeException>(() =>
                AuditBundleWriter.Write(destination, CreateResult(), options: options));

            Assert.Equal("preserve", File.ReadAllText(markerPath));
            Assert.Equal(["bundle"], Directory.GetFileSystemEntries(parent).Select(Path.GetFileName));
            Assert.Equal(["marker.txt"], Directory.GetFileSystemEntries(destination).Select(Path.GetFileName));
        }
        finally
        {
            Directory.Delete(parent, recursive: true);
        }
    }

    [Fact]
    public void Write_FullProfile_WritesExactFullDisclosureBundle()
    {
        var destination = TempDestinationPath();
        var result = CreateResult();
        var assessment = result.Assess(AssessmentSpecification.Create(
            "residual-limit",
            rules => rules.MaximumResidualMagnitude(0, AssessmentAggregation.Total)));

        try
        {
            var bundle = AuditBundleWriter.Write(
                destination,
                result,
                assessment,
                [],
                new AuditBundleOptions { Profile = ArtifactExportProfile.Full });

            Assert.Equal(
                ["assessment.json", "manifest.json", "result.json", "traces.json"],
                Directory.GetFiles(destination).Select(Path.GetFileName).Order(StringComparer.Ordinal));
            var evidence = File.ReadAllText(Path.Combine(destination, "result.json"));
            Assert.Equal(result.ExportJson(), evidence);
            Assert.Contains("sensitive-device", evidence);
            Assert.Contains("private-source", evidence);
            Assert.Equal("[]", File.ReadAllText(Path.Combine(destination, "traces.json")));
            Assert.Equal(ArtifactExportProfile.Full, bundle.Manifest.Profile);
            Assert.Equal(
                ["assessment.json", "result.json", "traces.json"],
                bundle.Manifest.Files.Select(static file => file.Path));

            var opened = AuditBundleReader.Open(destination);
            Assert.Equal(ArtifactExportProfile.Full, opened.Manifest.Profile);
            Assert.True(opened.Verify().IsValid);
        }
        finally
        {
            DeleteDestinationParent(destination);
        }
    }

    [Fact]
    public void Write_RedactedProfile_WritesExactRedactedDisclosureBundle()
    {
        var destination = TempDestinationPath();
        var result = CreateResult();
        var assessment = result.Assess(AssessmentSpecification.Create(
            "residual-limit",
            rules => rules.MaximumResidualMagnitude(0, AssessmentAggregation.Total)));
        var trace = result.TraceRow(Assert.Single(result.ResidualRowsWithFinality()));

        try
        {
            var bundle = AuditBundleWriter.Write(
                destination,
                result,
                assessment,
                [trace],
                new AuditBundleOptions { Profile = ArtifactExportProfile.Redacted });

            Assert.Equal(
                ["manifest.json", "result.redacted.json"],
                Directory.GetFiles(destination).Select(Path.GetFileName).Order(StringComparer.Ordinal));
            var evidence = File.ReadAllText(Path.Combine(destination, "result.redacted.json"));
            Assert.Equal(result.ExportRedactedAgentContext(), evidence);
            Assert.DoesNotContain("sensitive-device", evidence);
            Assert.DoesNotContain("private-source", evidence);
            Assert.Equal(ArtifactExportProfile.Redacted, bundle.Manifest.Profile);
            var file = Assert.Single(bundle.Manifest.Files);
            Assert.Equal("result.redacted.json", file.Path);

            var opened = AuditBundleReader.Open(destination);
            Assert.Equal(ArtifactExportProfile.Redacted, opened.Manifest.Profile);
            Assert.True(opened.Verify().IsValid);
        }
        finally
        {
            DeleteDestinationParent(destination);
        }
    }

    private static ComparisonResult CreateResult()
    {
        var history = new WindowHistoryFixtureBuilder()
            .AddClosedWindow("Offline", "sensitive-device", 0, 10, source: "private-source")
            .AddClosedWindow("Offline", "sensitive-device", 0, 7, source: "comparison-source")
            .Build();

        return history.Compare("bundle")
            .Target("private-source", selector => selector.Source("private-source"))
            .Against("comparison-source", selector => selector.Source("comparison-source"))
            .Within(scope => scope.Window("Offline"))
            .Using(comparators => comparators.Residual())
            .Run();
    }

    private static string TempDestinationPath() =>
        Path.Combine(Path.GetTempPath(), "spanfold-tests-" + Guid.NewGuid().ToString("N"), "bundle");

    private static void DeleteDestinationParent(string destination)
    {
        var parent = Path.GetDirectoryName(destination);
        if (parent is not null && Directory.Exists(parent))
        {
            Directory.Delete(parent, recursive: true);
        }
    }
}
