using System.Text.Json;

using Spanfold.Artifacts.Comparison;

namespace Spanfold.Testing;

/// <summary>
/// Provides backward-compatible access to Spanfold contract fixture execution.
/// </summary>
public static class ContractFixtureRunner
{
    /// <summary>
    /// Validates, constructs, and executes a contract fixture.
    /// </summary>
    /// <param name="fixture">The contract fixture JSON root.</param>
    /// <returns>The comparison result produced by the fixture plan.</returns>
    /// <exception cref="ArgumentException">The fixture does not conform to the supported schema.</exception>
    public static ComparisonResult Run(JsonElement fixture)
    {
        return ComparisonFixtureRunner.Run(fixture);
    }
}
