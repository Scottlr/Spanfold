namespace Spanfold.Testing;

/// <summary>
/// Provides framework-neutral assertions for Spanfold comparison artifacts.
/// </summary>
public static class SpanfoldAssert
{
    /// <summary>
    /// Asserts that a comparison result is valid.
    /// </summary>
    /// <param name="result">The result to inspect.</param>
    /// <exception cref="SpanfoldAssertionException">Thrown when the result contains error diagnostics.</exception>
    public static void IsValid(ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        if (result.IsValid)
        {
            return;
        }

        throw new SpanfoldAssertionException("Expected a valid Spanfold result, but error diagnostics were present.");
    }

    /// <summary>
    /// Asserts that a comparison result contains no diagnostics.
    /// </summary>
    /// <param name="result">The result to inspect.</param>
    /// <exception cref="SpanfoldAssertionException">Thrown when any diagnostic is present.</exception>
    public static void HasNoDiagnostics(ComparisonResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        if (result.Diagnostics.Count == 0)
        {
            return;
        }

        throw new SpanfoldAssertionException("Expected no Spanfold diagnostics, but found " + result.Diagnostics.Count.ToString(System.Globalization.CultureInfo.InvariantCulture) + ".");
    }

    /// <summary>
    /// Asserts that a comparison result contains a diagnostic code.
    /// </summary>
    /// <param name="result">The result to inspect.</param>
    /// <param name="code">The diagnostic code to find.</param>
    /// <returns>The matching diagnostic.</returns>
    /// <exception cref="SpanfoldAssertionException">Thrown when the diagnostic code is missing.</exception>
    public static ComparisonPlanDiagnostic HasDiagnostic(
        ComparisonResult result,
        ComparisonPlanValidationCode code)
    {
        ArgumentNullException.ThrowIfNull(result);

        for (var i = 0; i < result.Diagnostics.Count; i++)
        {
            if (result.Diagnostics[i].Code == code)
            {
                return result.Diagnostics[i];
            }
        }

        throw new SpanfoldAssertionException("Expected Spanfold diagnostic " + code + ".");
    }

    /// <summary>
    /// Asserts that a comparison satisfies an assessment specification.
    /// </summary>
    public static void Meets(ComparisonResult result, AssessmentSpecification specification)
    {
        ArgumentNullException.ThrowIfNull(result);
        ArgumentNullException.ThrowIfNull(specification);
        Passes(result.Assess(specification));
    }

    /// <summary>
    /// Asserts that a comparison assessment passed.
    /// </summary>
    public static void Passes(ComparisonAssessment assessment)
    {
        ArgumentNullException.ThrowIfNull(assessment);
        if (assessment.Passed)
        {
            return;
        }

        var first = assessment.Violations[0];
        throw new SpanfoldAssertionException(
            "Expected assessment '"
            + assessment.Specification.Name
            + "' to pass, but "
            + assessment.Violations.Count.ToString(System.Globalization.CultureInfo.InvariantCulture)
            + " violation(s) were produced. First violation: "
            + first.Code
            + ".");
    }

    /// <summary>
    /// Asserts that every assessment in a suite passed.
    /// </summary>
    public static void Passes(AssessmentSuiteResult suite)
    {
        ArgumentNullException.ThrowIfNull(suite);
        if (suite.Passed)
        {
            return;
        }

        var failed = suite.Assessments.First(static assessment => !assessment.Passed);
        throw new SpanfoldAssertionException(
            "Expected assessment suite '"
            + suite.Suite.Name
            + "' to pass, but specification '"
            + failed.Specification.Name
            + "' failed.");
    }

    /// <summary>
    /// Asserts that a named row collection contains an expected number of rows.
    /// </summary>
    /// <param name="result">The result to inspect.</param>
    /// <param name="rowKind">The closed comparison row family.</param>
    /// <param name="expectedCount">The expected row count.</param>
    /// <exception cref="SpanfoldAssertionException">Thrown when the row count differs.</exception>
    public static void HasRowCount(ComparisonResult result, ComparisonRowKind rowKind, int expectedCount)
    {
        ArgumentNullException.ThrowIfNull(result);
        ArgumentOutOfRangeException.ThrowIfNegative(expectedCount);

        var actualCount = GetRowCount(result, rowKind);
        if (actualCount == expectedCount)
        {
            return;
        }

        throw new SpanfoldAssertionException(
            "Expected "
            + expectedCount.ToString(System.Globalization.CultureInfo.InvariantCulture)
            + " "
            + rowKind.ToArtifactLabel()
            + " rows, but found "
            + actualCount.ToString(System.Globalization.CultureInfo.InvariantCulture)
            + ".");
    }

    private static int GetRowCount(ComparisonResult result, ComparisonRowKind rowKind)
    {
        return rowKind switch
        {
            ComparisonRowKind.Overlap => result.OverlapRows.Count,
            ComparisonRowKind.Residual => result.ResidualRows.Count,
            ComparisonRowKind.Missing => result.MissingRows.Count,
            ComparisonRowKind.Coverage => result.CoverageRows.Count,
            ComparisonRowKind.Gap => result.GapRows.Count,
            ComparisonRowKind.SymmetricDifference => result.SymmetricDifferenceRows.Count,
            ComparisonRowKind.Containment => result.ContainmentRows.Count,
            ComparisonRowKind.LeadLag => result.LeadLagRows.Count,
            ComparisonRowKind.AsOf => result.AsOfRows.Count,
            _ => throw new ArgumentOutOfRangeException(nameof(rowKind), rowKind, "Unknown Spanfold row kind.")
        };
    }
}
