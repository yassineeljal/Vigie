import { useQuery } from "@tanstack/react-query";
import { commands } from "../../lib/ipc";
import { unwrap } from "../../lib/result";

export const useScoreReport = () =>
  useQuery({ queryKey: ["score"], queryFn: () => unwrap(commands.getScoreReport()) });
