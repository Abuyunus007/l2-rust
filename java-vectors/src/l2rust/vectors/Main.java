package l2rust.vectors;

import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Entry point of the vector generators. Each generator calls the real L2JMobius classes (compiled from the Java source tree) and writes JSON files.
 *
 * <pre>
 * java l2rust.vectors.Main &lt;generator|all&gt; [outDir]
 * </pre>
 *
 * Generators: {@code crypto}. Later cards add {@code login-packets}, {@code geo}, ...<br>
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
			System.err.println("usage: Main <crypto|all> [outDir]");
			System.exit(2);
		}

		final Path out = Paths.get(args.length > 1 ? args[1] : "java-vectors/out");
		switch (args[0])
		{
			case "crypto" -> CryptoVectors.generate(out);
			case "all" -> CryptoVectors.generate(out); // Add new generators here.
			default ->
			{
				System.err.println("unknown generator: " + args[0]);
				System.exit(2);
			}
		}
	}
}
