using Spanfold;

namespace Spanfold.Tests.Comparison;

public sealed class ComparisonComparatorCatalogTests
{
    [Fact]
    public void CatalogListsBuiltInComparatorDeclarations()
    {
        Assert.Equal(
            [
                "overlap",
                "residual",
                "missing",
                "coverage",
                "gap",
                "symmetric-difference",
                "containment"
            ],
            ComparisonComparatorCatalog.BuiltInDeclarations);
    }

    [Theory]
    [InlineData("overlap")]
    [InlineData("lead-lag:Start:ProcessingPosition:5")]
    [InlineData("lead-lag:End:Timestamp:9223372036854775807")]
    [InlineData("asof:Previous:ProcessingPosition:10")]
    [InlineData("asof:Next:Timestamp:0")]
    [InlineData("asof:Nearest:ProcessingPosition:+5")]
    public void KnownDeclarationsIncludeParameterizedCoreComparators(string declaration)
    {
        Assert.True(ComparisonComparatorCatalog.IsKnownDeclaration(declaration));
    }

    [Theory]
    [InlineData("Overlap")]
    [InlineData("lead-lag:start:ProcessingPosition:5")]
    [InlineData("lead-lag:Start:Unknown:5")]
    [InlineData("lead-lag:Start:ProcessingPosition:1,000")]
    [InlineData("asof:Previous:ProcessingPosition:10.0")]
    [InlineData("asof:Previous:ProcessingPosition:-1")]
    [InlineData("asof:Previous:ProcessingPosition:10:extra")]
    [InlineData("quality:drift")]
    public void UnknownDeclarationsAreNotClaimedByCoreCatalog(string declaration)
    {
        Assert.False(ComparisonComparatorCatalog.IsKnownDeclaration(declaration));
    }

    [Theory]
    [InlineData("overlap", true)]
    [InlineData("Overlap", false)]
    [InlineData("lead-lag:Start:ProcessingPosition:5", false)]
    public void BuiltInDeclarationsRequireAnExactBuiltInName(string declaration, bool expected)
    {
        Assert.Equal(expected, ComparisonComparatorCatalog.IsBuiltInDeclaration(declaration));
    }
}
