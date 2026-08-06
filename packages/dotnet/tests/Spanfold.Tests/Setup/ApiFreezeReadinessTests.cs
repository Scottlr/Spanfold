using System.Xml.Linq;

namespace Spanfold.Tests.Setup;

public sealed class ApiFreezeReadinessTests
{
    [Theory]
    [InlineData("src/Spanfold/Spanfold.csproj")]
    [InlineData("src/Spanfold.Artifacts/Spanfold.Artifacts.csproj")]
    [InlineData("src/Spanfold.Testing/Spanfold.Testing.csproj")]
    public void PackableProjectsEnforcePublicXmlDocumentation(string projectPath)
    {
        var project = LoadProject(projectPath);

        Assert.Equal("true", GetProperty(project, "GenerateDocumentationFile"));
        Assert.Contains("CS1591", GetProperty(project, "WarningsAsErrors"), StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("src/Spanfold/Spanfold.csproj", "true")]
    [InlineData("src/Spanfold.Artifacts/Spanfold.Artifacts.csproj", "true")]
    [InlineData("src/Spanfold.Testing/Spanfold.Testing.csproj", "true")]
    [InlineData("src/Spanfold.Cli/Spanfold.Cli.csproj", "false")]
    [InlineData("benchmarks/Spanfold.Benchmarks/Spanfold.Benchmarks.csproj", "false")]
    public void PackageBoundariesAreExplicit(string projectPath, string expectedPackable)
    {
        var project = LoadProject(projectPath);

        Assert.Equal(expectedPackable, GetProperty(project, "IsPackable"));
    }

    private static XDocument LoadProject(string projectPath)
    {
        return XDocument.Load(Path.Combine(RepositoryRoot(), projectPath));
    }

    private static string GetProperty(XDocument project, string name)
    {
        return project.Root!
            .Elements("PropertyGroup")
            .Elements(name)
            .Select(static element => element.Value)
            .FirstOrDefault() ?? string.Empty;
    }

    private static string RepositoryRoot()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null && !File.Exists(Path.Combine(directory.FullName, "Spanfold.slnx")))
        {
            directory = directory.Parent;
        }

        return directory?.FullName ?? throw new InvalidOperationException("Repository root was not found.");
    }
}
