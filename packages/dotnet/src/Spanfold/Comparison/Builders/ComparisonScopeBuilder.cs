namespace Spanfold.Comparison;

/// <summary>
/// Builds comparison scopes.
/// </summary>
public sealed class ComparisonScopeBuilder
{
    /// <summary>
    /// Uses all recorded windows on the processing-position axis.
    /// </summary>
    /// <returns>An unrestricted comparison scope.</returns>
    public ComparisonScope All()
    {
        return ComparisonScope.All();
    }

    /// <summary>
    /// Restricts the comparison to one configured window.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <returns>A window-name comparison scope.</returns>
    public ComparisonScope Window(string windowName)
    {
        return ComparisonScope.Window(windowName);
    }

    /// <summary>
    /// Restricts the comparison to one configured window on the supplied temporal axis.
    /// </summary>
    /// <param name="windowName">The configured window name.</param>
    /// <param name="timeAxis">The temporal axis used by the comparison.</param>
    /// <returns>A window-name comparison scope on the supplied axis.</returns>
    public ComparisonScope Window(string windowName, TemporalAxis timeAxis)
    {
        return ComparisonScope.Window(windowName, timeAxis);
    }
}
