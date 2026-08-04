using Spanfold.Internal.Definitions;

namespace Spanfold;

/// <summary>
/// Configures optional behavior for one source window.
/// </summary>
/// <typeparam name="TEvent">The event type consumed by the pipeline.</typeparam>
/// <typeparam name="TKey">The key type used by the source window.</typeparam>
public sealed class WindowOptions<TEvent, TKey>
    where TKey : notnull
{
    private readonly WindowCallbackSet<TEvent> callbacks;
    private Func<TEvent, bool>? exitPredicate;
    private int enterConfirmationCount = 1;
    private int exitConfirmationCount = 1;

    internal WindowOptions(WindowCallbackSet<TEvent> callbacks)
    {
        this.callbacks = callbacks;
    }

    /// <summary>
    /// Requires consecutive enter and exit observations before committing transitions.
    /// </summary>
    /// <param name="exitWhen">Returns true when an active window is eligible to close.</param>
    /// <param name="enterAfter">Consecutive active observations required to open.</param>
    /// <param name="exitAfter">Consecutive exit observations required to close.</param>
    /// <returns>The current options object.</returns>
    /// <remarks>
    /// The active predicate supplied when the window is created is the enter predicate.
    /// Transitions use the event that reaches the configured count as their boundary.
    /// </remarks>
    public WindowOptions<TEvent, TKey> Stabilize(
        Func<TEvent, bool> exitWhen,
        int enterAfter = 1,
        int exitAfter = 1)
    {
        ArgumentNullException.ThrowIfNull(exitWhen);
        ArgumentOutOfRangeException.ThrowIfLessThan(enterAfter, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(exitAfter, 1);

        this.exitPredicate = exitWhen;
        this.enterConfirmationCount = enterAfter;
        this.exitConfirmationCount = exitAfter;
        return this;
    }

    internal void ApplyTo(WindowDefinition<TEvent> definition)
    {
        if (this.exitPredicate is null)
        {
            return;
        }

        definition.ConfigureStabilization(
            this.exitPredicate,
            this.enterConfirmationCount,
            this.exitConfirmationCount);
    }

    /// <summary>
    /// Registers a callback invoked when this window opens.
    /// </summary>
    /// <param name="callback">The callback to invoke for open emissions.</param>
    /// <returns>The current options object.</returns>
    public WindowOptions<TEvent, TKey> OnOpened(Action<WindowEmission<TEvent>> callback)
    {
        ArgumentNullException.ThrowIfNull(callback);

        this.callbacks.Opened.Add(callback);
        return this;
    }

    /// <summary>
    /// Registers a callback invoked when this window closes.
    /// </summary>
    /// <param name="callback">The callback to invoke for close emissions.</param>
    /// <returns>The current options object.</returns>
    public WindowOptions<TEvent, TKey> OnClosed(Action<WindowEmission<TEvent>> callback)
    {
        ArgumentNullException.ThrowIfNull(callback);

        this.callbacks.Closed.Add(callback);
        return this;
    }
}
