namespace HaloSettings.Services;

public enum Side
{
    Left,
    Top,
    Right,
    Bottom,
}

/// <summary>One LED's span along a screen edge, normalised 0..1 (y grows downwards).</summary>
public readonly record struct Slot(Side Side, double A, double B)
{
    public (double X, double Y) Center
    {
        get
        {
            var m = (A + B) * 0.5;
            return Side switch
            {
                Side.Left => (0, m),
                Side.Top => (m, 0),
                Side.Right => (1, m),
                _ => (m, 1),
            };
        }
    }
}

/// <summary>Mirror of the service's LED order: clockwise from the bottom-left corner.</summary>
public readonly record struct StripLayout(int Left, int Top, int Right, int Bottom, bool Reverse)
{
    public int Total => Left + Top + Right + Bottom;

    public IReadOnlyList<Slot> Slots()
    {
        static IEnumerable<Slot> Run(Side side, int n, bool rising) =>
            Enumerable.Range(0, Math.Max(n, 0)).Select(i =>
            {
                double a = (double)i / n, b = (double)(i + 1) / n;
                return rising ? new Slot(side, a, b) : new Slot(side, 1 - b, 1 - a);
            });

        var all = Run(Side.Left, Left, false)
            .Concat(Run(Side.Top, Top, true))
            .Concat(Run(Side.Right, Right, true))
            .Concat(Run(Side.Bottom, Bottom, false))
            .ToList();
        if (Reverse)
        {
            all.Reverse();
        }
        return all;
    }
}
