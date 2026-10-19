package l2rust.vectors;

import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Entry point of the vector generators. Each generator calls the real L2JMobius classes (compiled from the Java source tree) and writes JSON files.
 *
 * <pre>
 * java l2rust.vectors.Main &lt;generator|all&gt; [outDir]
 * java l2rust.vectors.Main geo [outDir] &lt;geodataDownloadDir&gt; &lt;geodataConvertedDir&gt;
 * java l2rust.vectors.Main geo-engine [outDir] &lt;geodataDownloadDir&gt; &lt;geodataConvertedDir&gt;
 * java l2rust.vectors.Main geo-path [outDir] &lt;geodataDownloadDir&gt; &lt;geodataConvertedDir&gt;
 * (cwd = game server root, absolute outDir) java l2rust.vectors.Main stats|formulas|drops &lt;absolute outDir&gt;
 * </pre>
 *
 * Generators: {@code crypto}, {@code login-packets}, {@code geo}, {@code geo-engine} and {@code geo-path} (the last three need the two geodata folders as arguments, so {@code all} does not run them), {@code stats}, {@code formulas} and {@code drops} (need the game server root as working directory, so {@code all} does not run them either).<br>
 * {@code outDir} defaults to {@code java-vectors/out} (relative to the working directory).
 */
public final class Main
{
	private Main()
	{
	}

	public static void main(String[] args) throws Exception
	{
		if (args.length < 1)
		{
			System.err.println("usage: Main <crypto|login-packets|all> [outDir]");
			System.err.println("       Main geo|geo-engine|geo-path [outDir] <geodataDownloadDir> <geodataConvertedDir>");
			System.exit(2);
		}

		final Path out = Paths.get(args.length > 1 ? args[1] : "java-vectors/out");
		switch (args[0])
		{
			case "crypto" -> CryptoVectors.generate(out);
			case "geo" ->
			{
				if (args.length < 4)
				{
					System.err.println("usage: Main geo [outDir] <geodataDownloadDir> <geodataConvertedDir>");
					System.exit(2);
				}
				GeoVectors.generate(out, Paths.get(args[2]), Paths.get(args[3]));
			}
			case "geo-engine" ->
			{
				if (args.length < 4)
				{
					System.err.println("usage: Main geo-engine [outDir] <geodataDownloadDir> <geodataConvertedDir>");
					System.exit(2);
				}
				GeoEngineVectors.generate(out, Paths.get(args[2]), Paths.get(args[3]));
			}
			case "geo-path" ->
			{
				if (args.length < 4)
				{
					System.err.println("usage: Main geo-path [outDir] <geodataDownloadDir> <geodataConvertedDir>");
					System.exit(2);
				}
				GeoPathVectors.generate(out, Paths.get(args[2]), Paths.get(args[3]));
			}
			case "stats", "formulas", "drops" ->
			{
				// The working directory is the game server root (config/ and data/ are read from it); a relative outDir would end up there.
				if (!out.isAbsolute())
				{
					System.err.println("usage: Main " + args[0] + " <absolute outDir>   (working directory = game server root)");
					System.exit(2);
				}
				try
				{
					if (args[0].equals("stats"))
					{
						StatVectors.generate(out);
					}
					else if (args[0].equals("drops"))
					{
						DropVectors.generate(out);
					}
					else
					{
						FormulaVectors.generate(out);
					}
				}
				catch (Throwable t)
				{
					// The server's thread pools are not daemon threads: without an explicit exit a failed run would hang.
					t.printStackTrace();
					System.exit(1);
				}
			}
			case "login-packets" -> LoginPacketVectors.generate(out);
			case "all" ->
			{
				// Add new generators here (geo needs its own arguments, see above).
				CryptoVectors.generate(out);
				LoginPacketVectors.generate(out);
			}
			default ->
			{
				System.err.println("unknown generator: " + args[0]);
				System.exit(2);
			}
		}
	}
}
