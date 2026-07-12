namespace Spanfold.Internal.Runtime;

internal sealed class RuntimeMutationJournal
{
    private readonly List<Action> undo = [];

    internal void Set<TKey, TValue>(Dictionary<TKey, TValue> dictionary, TKey key, TValue value)
        where TKey : notnull
    {
        var existed = dictionary.TryGetValue(key, out var previous);
        this.undo.Add(() =>
        {
            if (existed)
            {
                dictionary[key] = previous!;
            }
            else
            {
                dictionary.Remove(key);
            }
        });
        dictionary[key] = value;
    }

    internal bool Remove<TKey, TValue>(Dictionary<TKey, TValue> dictionary, TKey key)
        where TKey : notnull
    {
        if (!dictionary.Remove(key, out var previous))
        {
            return false;
        }

        this.undo.Add(() => dictionary[key] = previous);
        return true;
    }

    internal void Record(Action undoAction)
    {
        this.undo.Add(undoAction);
    }

    internal void Rollback()
    {
        for (var i = this.undo.Count - 1; i >= 0; i--)
        {
            this.undo[i]();
        }

        this.undo.Clear();
    }

    internal void Commit()
    {
        this.undo.Clear();
    }
}
