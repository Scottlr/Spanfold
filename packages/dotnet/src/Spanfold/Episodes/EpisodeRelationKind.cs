namespace Spanfold.Episodes;

/// <summary>
/// Classifies one connected component in an episode relation graph.
/// </summary>
public enum EpisodeRelationKind
{
    /// <summary>One target episode relates to one against episode.</summary>
    OneToOne = 0,

    /// <summary>One target episode relates to two or more against episodes.</summary>
    Split = 1,

    /// <summary>Two or more target episodes relate to one against episode.</summary>
    Merge = 2,

    /// <summary>Two or more episodes occur on both sides.</summary>
    Complex = 3,

    /// <summary>One target episode has no related against episode.</summary>
    UnmatchedTarget = 4,

    /// <summary>One against episode has no related target episode.</summary>
    UnmatchedAgainst = 5
}
